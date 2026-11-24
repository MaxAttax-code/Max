use std::{
    cell::{Cell}, collections::{HashMap, HashSet}, sync::Arc,
};

use max_bytecode::{self as bc, FieldInfo, MethodInfo};

use crate::{ast::*, error::MaxDiagnostic};

fn error(span: Span, code: &'static str, message: impl Into<String>) -> MaxDiagnostic {
    MaxDiagnostic::new(crate::error::Severity::Error, code, message.into(), span)
}

// ---------------------------- //
// ----- SHARED CONTEXT ------- //
// ---------------------------- //

// When we say shared, think of the keyword 'using'

/// The names the compiler needs from the native registry
#[derive(Debug, Default, Clone)]
pub struct NativeNames {
    pub module_tasks: HashSet<String>,
    pub module_properties: HashSet<String>,
    pub global_tasks: HashSet<String>,
    pub transmutation_tasks: HashSet<String>
}

/**
A parameter as the *caller* needs to see it: just a name and a kind.

The compiler needs this (and not the full AST `Parameter`) because binding arguments
only cares about "what is it called" and "is it required / optional / variadic".
Binding arguments here means attaching an argument value with it's parameter name.
Types are still checked at runtime by the VM, so they are not copied here.
*/
#[derive(Debug, Clone)]
pub struct ParamSignature { pub name: String, pub kind: ParameterKind }


/// Summarizes whether a task can return monic data and which parameters it may forward.
/// 
/// Keep in mind that a task keeps record not only of monic values within that are being
/// returned, but **also parameters passed in by monic values**, which hold their monomorphism
/// up to the being returned (`forwarded_parameters` represents these monic parameters)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonicReturnSummary {
    /// True if at least one return path can produce a monic value independent of its arguments.
    /// Despite being called `always_monic`, this refers to whether the task returns a monic
    /// value outside it's arguments passed in
    pub always_monic: bool,
    /// Zero-based parameter positions whose values may be returned directly.
    pub forwarded_parameters: HashSet<usize>,
}

/// Provenance of an expression while analyzing a task body.
/// This is identical to [`MonicReturnSummary`], but serves
/// to represent the original state an expression was in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MonicOrigin {
    /// The expression may originate from a monic local or a monic return.
    always_monic: bool,
    /// The expression may directly forward one or more task parameters.
    /// Know that arguments passed in that are **not** monic are treated
    /// as ordinary parameters, known as the provenance
    forwarded_parameters: HashSet<usize>,
}

impl MonicOrigin {
    /// Returns whether this expression may directly transfer monic-origin data.
    fn is_monic(&self) -> bool {
        self.always_monic || !self.forwarded_parameters.is_empty()
    }

    /// Combines provenance from multiple parts of an expression or return paths.
    fn merge(&mut self, other: Self) {
        // What is THAT? Why of course the bits-OR operator!'
        // [Do not always trust random links, check them first]
        // Search it up: https://www.geeksforgeeks.org/dsa/complete-reference-for-bitwise-operators-in-programming-coding/#2-bitwise-or-operator-
        self.always_monic |= other.always_monic;
        self.forwarded_parameters.extend(other.forwarded_parameters);
    }
}

impl MonicReturnSummary {
    /// Adds another set of possible return origins to this task's summary.
    fn merge(&mut self, other: Self) {
        self.always_monic |= other.always_monic;
        self.forwarded_parameters.extend(other.forwarded_parameters);
    }
}

/**
This struct is essentially the ANSWER to the puzzle "which argument feeds which parameter".
It holds indexes into the call's `arguments` list, never the expressions themselves.

```text
foo(b = 2, 1)  with params (a, b)
    arguments: [0] b = 2     [1] 1
    Binding {
        fixed: [
            Some(1),
            ^ a gets arguments[1]
            Some(0)
            ^ b gets arguments[0]
        ],
        variadic: []
    }
```
*/
struct Binding {
    /// Per fixed parameter: index into `arguments`, or None (it becomes nil)
    fixed: Vec<Option<usize>>,
    /// Arguments feeding the variadic parameter, in source order
    variadic: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum SymbolInfo {
    Task(Vec<ParamSignature>),
    Coroutine,
    Event { parameters: Vec<ParamSignature> },
    Enum(Vec<EnumMember>),
    Object(ObjectSignature),
}

/**
Records what a module exposes to the files that are `using` it, including each task's
parameter names so those files can bind named arguments.

Events are special: the same event name may be defined many times (one handler each),
so every handler's parameters are APPENDED to one flat list, in definition order.
That flat list is exactly the "payload positions" `emit` fills.

Be careful when editing this: every arm must only touch the definition it matched.
(An earlier draft inserted an `Event` entry for every task and used `break`, which
ended the loop after the first definition. Silly billy!)
*/
#[derive(Debug, Clone)]
pub struct SymbolTable {
    pub module_name: String,
    pub contents: HashMap<String, SymbolInfo>,
    /// The tasks that return monic values, held in `MonicReturnSummary`s
    pub monic_returns: HashMap<String, MonicReturnSummary>,
}

impl SymbolTable {
    /// Builds a module's exported declarations and analyzes task return provenance.
    /// The provenance is the original state the task return was in. Once it is found
    /// that the task returns
    ///
    /// `dependencies` and `symbols` let the analysis follow task calls into imported modules.
    pub fn from_statements(
        module_name: &str,
        statements: &[Statement],
        dependencies: &[String],
        symbols: &HashMap<String, SymbolTable>,
    ) -> Self {
        let mut contents = HashMap::new();
        // Our monic returns come from this very important fn
        let monic_returns = monic_returning_tasks(statements, dependencies, symbols);

        for statement in statements {
            match &statement.kind {
                StatementKind::TaskDefinition(task) => match task.kind {
                    TaskKind::Function => { contents.insert(task.name.clone(), SymbolInfo::Task(signature_of_task(task))); }
                    TaskKind::Coroutine => { contents.insert(task.name.clone(), SymbolInfo::Coroutine); }
                    TaskKind::Event => {
                        if let SymbolInfo::Event { parameters } = contents
                            .entry(task.name.clone())
                            .or_insert(SymbolInfo::Event { parameters: Vec::new() })
                        {
                            parameters.extend(signature_of_task(task));
                        }
                    }
                },

                StatementKind::Enum(definition) => {
                    contents.insert(definition.identifier.clone(), SymbolInfo::Enum(definition.members.clone()));
                }

                StatementKind::Model(model) => match model {
                    ModelDefinition::Object(object) =>
                        { contents.insert(object.name.node.clone(), SymbolInfo::Object(ObjectSignature::from_definition(object))); }
                },

                _ => {}
            }
        }

        SymbolTable { module_name: module_name.to_string(), contents, monic_returns }
    }
}

/// State shared by every file and lambda in one compilation.
/// The counters are unique program-wide.
pub struct CompileContext {
    pub natives: NativeNames,
    /// Add a module's table here before compiling it (and before the files that use it)
    pub symbols: HashMap<String, SymbolTable>,
    next_lambda_id: Cell<u64>,
    next_temporary_slot: Cell<u64>,
}

impl CompileContext {
    pub fn new(natives: NativeNames) -> Self {
        CompileContext {
            natives,
            symbols: HashMap::new(),
            next_lambda_id: Cell::new(0),
            next_temporary_slot: Cell::new(0),
        }
    }

    /// Allocates a lambda ID, returning the ID to be given, and incrementing `next_lambda_id` by 1
    fn alloc_lambda_id(&self) -> u64 {
        let id = self.next_lambda_id.get();
        self.next_lambda_id.set(id + 1);
        id
    }

    /// Allocates a temporary slot, returning the given slot index back, and incrementing the `next_temporary_slot` by 1
    fn alloc_slot(&self) -> u64 {
        let slot = self.next_temporary_slot.get();
        self.next_temporary_slot.set(slot + 1);
        slot
    }
}

// ---------------------------- //
// --------- COMPILER --------- //
// ---------------------------- //

#[derive(Debug, Clone, Copy, Default)]
struct VarInfo {
    /// Declared with `open`, so it may be shadowed
    open: bool,
    /// Declared with `monic`, so it cannot be involved in assignments
    monic: bool,
    /// Declared with `fixed`, so it can not be reassigned or mutated in any way
    fixed: bool
}

/// What kind of Enum Node
enum NodeKind {
    Definition,
    Member,
}

/// What kind of member of a model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Field,
    Task
}

/// Holds the signature of a member. It holds
/// it's attributes and name, but also it's `MemberKind`
/// to differentiate different member types
#[derive(Debug, Clone)]
pub struct MemberSignature {
    pub name: String,
    pub kind: MemberKind,
    pub is_static: bool,
    pub visibility: Visibility
}

/// Holds the signature of an Object:
/// - It's `name`
/// - It's `parameters`
/// - Any `members` it has as *`MemberSignature`s*
/// 
/// Signatures allow models and their members to supply
/// information to dependers
#[derive(Debug, Clone)]
pub struct ObjectSignature {
    pub name: String,
    pub parameters: Vec<ParamSignature>,
    pub members: Vec<MemberSignature>
}

impl ObjectSignature {
    pub fn from_definition(definition: &ObjectDefinition) -> Self {
        let name = definition.name.node.clone();
        // Convert to signatures
        let parameters = definition.parameters.iter().map(
            |p| ParamSignature {
                name: p.name.node.clone(),
                kind: p.kind.clone()
            }
        ).collect();
        let members: Vec<MemberSignature> = definition.members.iter().map(
            |mem| {
                match mem {
                    ModelMember::Field(f_mem) => MemberSignature {
                        name: f_mem.declaration.name.clone(),
                        kind: MemberKind::Field,
                        is_static: f_mem.attributes.is_static,
                        // Our default public is in play here
                        visibility: f_mem.attributes.visibility.unwrap_or(Visibility::Public)
                    },
                    
                    ModelMember::Task(t_mem) => MemberSignature {
                        name: t_mem.definition.name.clone(),
                        kind: MemberKind::Task,
                        is_static: t_mem.attributes.is_static,
                        // Our default public is in play here
                        visibility: t_mem.attributes.visibility.unwrap_or(Visibility::Public)
                    }
                }
            }
        ).collect();

        Self { name, parameters, members }
    }
}

/// The Jump Kind of a jump.
/// Loops allow break or skip.
/// Match only allows break
#[derive(Clone, Copy, PartialEq, Eq)]
enum JumpKind { Loop, Match }

/// Gives context as to what a statements jumps lead to
struct JumpContext {
    kind: JumpKind,
    /// Runtime scope depth at the point a `break` lands
    break_depth: usize,
    /// Runtime scope depth at the point a `skip` lands. Specific to loops
    skip_depth: usize,
    /// Each position a break is in
    break_jumps: Vec<usize>,
    /// Each position a skip is in
    skip_jumps: Vec<usize>,
}

/// What an enum expression statically refers to.
/// Holds a path to it's location, and it's kind/children,
/// such as `ItemType` or `ItemType.Tool`
struct EnumNode<'s> {
    kind: NodeKind,
    path: String,
    children: &'s [EnumMember],
}

/// A structure containing both the event name and arity.
/// This is how a module learns of the arity of an event from another module
#[derive(Clone)]
struct EventHandler { task_name: String, parameter_count: usize }

/// Struct that gives context into all fields and tasks inside the model.
/// 
/// To find the members (fields and tasks) within, simply use the `name`
/// to look for the object in `local_objects` and access from there.
#[derive(Clone)]
struct ModelContext {
    name: String,
    /// Used to decide when the 'this' expression can be used
    in_instance_task: bool,
    /// Used to decide when to prevent 'this' expressions
    in_static_task: bool,
}

pub struct CompileOutput {
    pub bytecode: Option<bc::Bytecode>,
    pub diagnostics: Vec<MaxDiagnostic>
}

pub struct Compiler<'a> {
    /// Our CompileContext for the Compiler
    ctx: &'a CompileContext,
    module_name: &'a str,
    dependencies: &'a [String],

    /// All the OpCodes for the `VirtualMachine` to read
    ops: Vec<bc::Op>,
    /// All the Diagnostics to be sent to the LSP
    diagnostics: Vec<MaxDiagnostic>,

    tasks: Arc<HashMap<String, bc::TaskInfo>>,
    local_tasks: HashMap<String, Vec<ParamSignature>>,
    /// A map of all tasks that can possibly return a monic value (our `MonicReturnSummary`)
    local_monic_returns: HashMap<String, MonicReturnSummary>,

    objects: Arc<HashMap<String, bc::ObjectStructure>>,
    local_objects: HashMap<String, ObjectSignature>,

    model_context: Option<ModelContext>,

    coroutines: Arc<HashMap<String, bc::CoroutineInfo>>,
    enums: Arc<HashMap<String, bc::EnumStructure>>,

    /**
        All the jump contexts within the file.
        Each statement that requires jumps to loop or move (e.g. `For`, `While`) holds
        jump context to provide info for `break` and `skip+`
    */
    jump_contexts: Vec<JumpContext>,

    /// Every enum in this file (collected before compiling), as the parser produced them
    local_enums: HashMap<String, Vec<EnumMember>>,

    /// (Name -> handlers) compiled so far, in definition order (used to build the dispatcher)
    events: HashMap<String, Vec<EventHandler>>,
    /// (Name -> total parameter count) across every handler in this file. Filled in pass 1, so
    /// an `emit` can appear before the handlers it fires, and clones into lambda children.
    local_events: HashMap<String, usize>,

    /// Each level has a boolean, which determines if the task is returnable from or not.
    /// False at the top level (first element)
    task_depth: Vec<bool>,
    variable_scopes: Vec<HashMap<String, VarInfo>>,
}

impl<'a> Compiler<'a> {
    pub fn new(ctx: &'a CompileContext, module_name: &'a str, dependencies: &'a [String]) -> Self {
        Compiler {
            ctx,
            module_name,
            dependencies,
            ops: Vec::new(),
            diagnostics: Vec::new(),
            tasks: Arc::default(),
            local_tasks: HashMap::new(),
            local_monic_returns: HashMap::new(),
            objects: Arc::default(),
            local_objects: HashMap::new(),
            model_context: None,
            coroutines: Arc::default(),
            enums: Arc::default(),
            local_enums: HashMap::new(),
            events: HashMap::new(),
            local_events: HashMap::new(),
            jump_contexts: Vec::new(),
            task_depth: vec![false],
            variable_scopes: vec![HashMap::new()],
        }
    }

    /**
        A compiler for a lambda body. It sees what the parent has defined *so far*,
        but its instructions go into its own list.
        Keep in mind that `jump_contexts` is not saved into the lambda,
        hence you cannot break or skip a lambda
    */
    fn new_child(&self) -> Compiler<'a> {
        Compiler {
            ctx: self.ctx,
            module_name: self.module_name,
            dependencies: self.dependencies,
            ops: Vec::new(),
            diagnostics: Vec::new(),
            tasks: self.tasks.clone(),
            local_tasks: self.local_tasks.clone(),
            local_monic_returns: self.local_monic_returns.clone(),
            objects: self.objects.clone(),
            local_objects: self.local_objects.clone(),
            model_context: self.model_context.clone(),
            coroutines: self.coroutines.clone(),
            enums: self.enums.clone(),
            local_enums: self.local_enums.clone(),
            events: HashMap:: new(),
            local_events: self.local_events.clone(),
            jump_contexts: Vec::new(),
            task_depth: vec![false, true],
            variable_scopes: vec![HashMap::new()],
        }
    }

    /// Compiles from the given array of statements.
    /// This is also where all local fields are filled
    pub fn compile(mut self, statements: &[Statement]) -> CompileOutput {
        let mut diagnostics: Vec<MaxDiagnostic> = Vec::new();
        self.local_monic_returns = monic_returning_tasks(statements, self.dependencies, &self.ctx.symbols);

        // Pass 1: learn every top-level enum, so types and member chains can refer to enums defined later
        for statement in statements {
            match &statement.kind {
                StatementKind::Enum(r#enum) => { self.local_enums.insert(r#enum.identifier.clone(), r#enum.members.clone()); },

                StatementKind::TaskDefinition(task) => {
                    match task.kind {
                         // Same with events, we want all the arity learned first
                        TaskKind::Event => { *self.local_events.entry(task.name.clone()).or_insert(0) += task.parameters.len(); }
                        TaskKind::Function => { self.local_tasks.entry(task.name.clone()).or_insert(Vec::new()).extend(signature_of_task(task)); }

                        _ => {}
                    }
                }

                StatementKind::Model(model) => {
                    match model {
                        ModelDefinition::Object(object) =>
                            { self.local_objects.insert(object.name.node.clone(), ObjectSignature::from_definition(object)); }
                    }
                }

                _ => {}
            }
        }

        // Pass 2: emit all definitions first, so they exist before any code runs
        for statement in statements {
            match &statement.kind {
                // 2a. Enums
                StatementKind::Enum(r#enum) => match self.compile_enum_definition(r#enum) {
                    Ok(_) => {},
                    Err(d) => diagnostics.push(d),
                }

                // 2b. Models
                StatementKind::Model(model) => match self.compile_model_definition(model) {
                    Ok(_) => {},
                    Err(d) => diagnostics.push(d),
                }

                _ => {}
            }
        }

        // Pass 3: everything else, in order
        for statement in statements {
            // These skip the ones that we filled from before
            if matches!(statement.kind, StatementKind::Enum(_)) ||
                matches!(statement.kind, StatementKind::Model(_)) {
                continue;
            }
            
            match self.compile_statement(statement) {
                Ok(_) => {},
                Err(d) => diagnostics.push(d),
            }
        }

        // Pass 4: one dispatcher per event, now that every handler is known
        let mut names: Vec<String> = self.events.keys().cloned().collect();
        names.sort(); // HashMap order is random; keep the bytecode deterministic
        for name in names {
            let handlers = self.events[&name].clone();
            self.emit_event_dispatcher(&name, &handlers);
        }

        let bytecode = bc::Bytecode {
            instructions: Arc::new(self.ops),
            tasks: Arc::clone(&self.tasks),
            objects: Arc::clone(&self.objects),
            coroutines: Arc::clone(&self.coroutines),
            enums: Arc::clone(&self.enums),
        };

        diagnostics.extend(self.diagnostics);
        CompileOutput { bytecode: Some(bytecode), diagnostics }
    }

    // ---------------------------- //
    // -------- EMIT HELPERS ------ //
    // ---------------------------- //

    /// Simply pushes an OpCode into the Compilers list
    fn emit(&mut self, op: bc::Op) {
        self.ops.push(op);
    }

    /// Emits a Constant OpCode specifically
    fn emit_constant(&mut self, constant: bc::ConstValue) {
        self.emit(bc::Op::Constant(constant));
    }

    /// Emits a jump with a placeholder target and returns its index, to be patched later
    fn emit_jump(&mut self, make: fn(usize) -> bc::Op) -> usize {
        self.ops.push(make(usize::MAX));
        // This works as push() adds to the end of the vec
        self.ops.len() - 1
    }

    /// Patches a jump, i.e. Find the jump by it's index, and setting the Jump's target to the given target.
    fn patch_jump(&mut self, index: usize, target: usize) {
        match &mut self.ops[index] {
            bc::Op::Jump(t) | bc::Op::JumpIfFalse(t) | bc::Op::JumpIfPossible(t) => *t = target,
            other => panic!("patch_jump was called on a non-jump instruction: {other:?}"),
        }
    }

    fn enter_compiler_scope(&mut self) {
        self.variable_scopes.push(HashMap::new());
    }

    fn exit_compiler_scope(&mut self) {
        if self.variable_scopes.len() > 1 {
            self.variable_scopes.pop();
        }
    }

    fn compile_statements(&mut self, statements: &[Statement]) -> Result<(), MaxDiagnostic> {
        for statement in statements {
            // Remember compiler state, since an early `?` can skip cleanup
            let scopes = self.variable_scopes.len();
            let task_depth = self.task_depth.clone();
            let jumps = self.jump_contexts.len();

            if let Err(diagnostic) = self.compile_statement(statement) {
                self.diagnostics.push(diagnostic);
                self.variable_scopes.truncate(scopes);
                self.task_depth = task_depth;
                self.jump_contexts.truncate(jumps);
            }
        }
        Ok(())
    }

    /// `EnterScope`, the body, `ExitScope`, with the compiler's own scopes kept in step
    fn compile_scoped_block(&mut self, body: &[Statement]) -> Result<(), MaxDiagnostic> {
        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();
        self.compile_statements(body)?;
        self.emit(bc::Op::ExitScope);
        self.exit_compiler_scope();
        Ok(())
    }

    /// Pushes a new `JumpContext` in, with the given `break_depth` and `skip_depth`.
    /// The `break_jumps` and `skip_jumps` are started as empty `Vec`
    fn push_jump_context(&mut self, kind: JumpKind, break_depth: usize, skip_depth: usize) {
        self.jump_contexts.push(JumpContext {
            kind, break_depth, skip_depth, break_jumps: Vec::new(), skip_jumps: Vec::new(),
        });
    }

    /// Pops from the `jump_contexts` list, returning the `JumpContext``
    fn pop_jump_context(&mut self) -> JumpContext {
        self.jump_contexts.pop().expect("push and pop are paired")
    }

    /// Compiles a `break` by closing all scopes up to `break_depth` of the last `JumpContext` in `jump_contexts`
    fn compile_break(&mut self, span: Span) -> Result<(), MaxDiagnostic> {
        // Smart checking by seeing if the break depth in the last jump context exists
        let Some(target) = self.jump_contexts.last().map(|c| c.break_depth) else {
            return Err(error(span, "break-outside-loop", "'break' can only be used inside a loop or a match statement"));
        };

        // Close every scope opened since the target, then leave
        for _ in 0..self.variable_scopes.len() - target {
            self.emit(bc::Op::ExitScope);
        }
        // Leap into action! Or actually out of action. Quick! Make a 'break' for it!
        let jump = self.emit_jump(bc::Op::Jump);
        self.jump_contexts.last_mut().unwrap().break_jumps.push(jump);
        Ok(())
    }

    /// Compiles a `skip` by closing the scopes currently opened and going to the beginning of a loop again.
    /// Match statements are ignored, so instead the nearest loop is skipped
    fn compile_skip(&mut self, span: Span) -> Result<(), MaxDiagnostic> {
        // A match is stepped over: `skip` belongs to the nearest enclosing loop, so matches are just totally done for!
        // Yes, keep in mind using `rposition` means we don't just check one position in the contexts!
        let Some(index) = self.jump_contexts.iter().rposition(|c| c.kind == JumpKind::Loop) else {
            return Err(error(span, "skip-outside-loop", "'skip' can only be used inside a loop"));
        };

        // Close the blowin scopes...
        for _ in 0..self.variable_scopes.len() - self.jump_contexts[index].skip_depth {
            self.emit(bc::Op::ExitScope);
        }
        // This time we ARE jumping into action!
        let jump = self.emit_jump(bc::Op::Jump);
        self.jump_contexts[index].skip_jumps.push(jump);
        Ok(())
    }

    // ---------------------------- //
    // ------- NAME CHECKS -------- //
    // ---------------------------- //

    fn is_variable_declared(&self, name: &str) -> bool {
        self.variable_scopes.iter().any(|scope| scope.contains_key(name))
    }

    /// Checks if a given expression holds a variable with the given kind. The kind must be
    /// **one of the fields** in `VarInfo`. This uses `break_member_access` to find a variable
    /// deeply rooted within an expression. Certain expressions like `foo()[0]` will evaluate
    /// to false regardless, as no variable is included in the expression.
    fn is_expression_of(&self, expression: &Expression, search_for: &str) -> bool {
        let found = Self::break_member_access(expression)
            .and_then(|name| self.variable_scopes.iter().rev().find_map(|scope| scope.get(name)));

        match search_for {
            "fixed" => found.is_some_and(|info| info.fixed),
            "open" => found.is_some_and(|info| info.open),
            "monic" => found.is_some_and(|info| info.monic),

            _ => false
        }
    }

    /// Computes whether an expression directly carries monic *provenance* in the current scope.
    /// Property reads and transforming calls are treated as new values; direct reads,
    /// arithmetic, indexing, and summarized task returns retain provenance.
    /// 
    /// ### Visualisation
    /// 
    /// ```
    /// let expression = Expression {
    ///     // Expression: monicValue + " is a monic value"
    /// }
    /// // This is will be true, as monicValue + " is a monic value" is treateed as monomorphic.
    /// // We do this because it is more favourable to do; monicValue += " is a monic value"
    /// assert!(expression_monic_origin(expression).always_monic);
    /// 
    /// let other_expression = Expression {
    ///     // Expression: monicValue.toUpper()
    /// }
    /// // monicValue.toUpper() produces a new value without any relation of transforming the monicValue.
    /// // To change the monicValue itself, you'd have to reassign itself to the new value!
    /// assert!(!expression_monic_origin(other_expression).monicValue)
    /// ```
    fn expression_monic_origin(&self, expression: &Expression) -> MonicOrigin {
        // We find variables of monomorphism
        let variables = self.variable_scopes.iter()
            .flat_map(|scope| scope.iter())
            // Produce MonicOrigins from the variable info
            .map(|(name, info)| (name.clone(), MonicOrigin { always_monic: info.monic, forwarded_parameters: HashSet::new() }))
            .collect();

        // This detects if the expression holds a monic origin/provenance
        monic_origin_in_expression(
            expression,
            &variables,
            &self.local_tasks,
            &self.local_monic_returns,
            self.dependencies,
            &self.ctx.symbols,
        )
    }

    /// Rejects passing a direct monic-origin value into a user-defined task.
    fn reject_monic_arguments(&self, arguments: &[Argument]) -> Result<(), MaxDiagnostic> {
        // Do not allow!!!
        if let Some(argument) = arguments.iter().find(|argument| self.expression_monic_origin(&argument.value).is_monic()) {
            return Err(error(argument.value.span, "monic-value-transfer", "A monic value cannot be transferred as a task argument"));
        }
        Ok(())
    }

    /// Ensures the name given is not a duplicate name with `has_duplicate_naming`,
    /// and also checks if any local events hold that name
    fn has_duplicate_naming(&self, name: &str) -> bool {
        self.local_events.contains_key(name) || self.non_event_name(name)
    }

    /// A task, coroutine, enum, native global or non-open variable already uses this name
    fn non_event_name(&self, name: &str) -> bool {
        if self.tasks.contains_key(name)
            || self.coroutines.contains_key(name)
            || self.enums.contains_key(name)
            || self.objects.contains_key(name)
            || self.ctx.natives.global_tasks.contains(name)
        {
            return true;
        }

        for scope in self.variable_scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                // Found in an outer scope: only `open` variables may be shadowed
                return !info.open;
            }
        }

        false
    }

    /// Names that are not variables: enums and modules. Lambdas must not try to capture these.
    fn is_compile_time_name(&self, name: &str) -> bool {
        !self.is_variable_declared(name)
            && (self.enums.contains_key(name)
                || self.find_enum_members(name).is_some()
                || self.resolve_object(name).is_some()
                || self.ctx.symbols.contains_key(name))
    }

    // ---------------------------- //
    // ---------- TYPES ----------- //
    // ---------------------------- //

    fn convert_type(&self, type_ref: &TypeRef, span: Span) -> Result<bc::MaxType, MaxDiagnostic> {
        Ok(match type_ref {
            TypeRef::Int => bc::MaxType::IntegerType,
            TypeRef::Float => bc::MaxType::FloatType,
            TypeRef::Str => bc::MaxType::StringType,
            TypeRef::Boolean => bc::MaxType::BooleanType,
            TypeRef::Coroutine => bc::MaxType::CoroutineType,

            TypeRef::List(element) => bc::MaxType::ListType(match element {
                Some(inner) => Some(Box::new(self.convert_type(inner, span)?)),
                None => None,
            }),

            TypeRef::Dictionary(key, value) => bc::MaxType::DictionaryType(
                match key {
                    Some(inner) => Some(Box::new(self.convert_type(inner, span)?)),
                    None => None,
                },
                match value {
                    Some(inner) => Some(Box::new(self.convert_type(inner, span)?)),
                    None => None,
                },
            ),

            TypeRef::Named(name) => {
                if self.find_enum_members(name).is_some() {
                    bc::MaxType::EnumType(name.clone())
                }
                else if self.resolve_object(name).is_some() {
                    bc::MaxType::ObjectType(name.clone())
                }
                else {
                    return Err(error(span, "unknown-type", format!("Unknown type '{name}'")));
                }
            }
        })
    }

    fn convert_optional_type(&self, type_ref: &Option<Spanned<TypeRef>>) -> Result<Option<bc::MaxType>, MaxDiagnostic> {
        match type_ref {
            Some(spanned) => Ok(Some(self.convert_type(&spanned.node, spanned.span)?)),
            None => Ok(None),
        }
    }

    fn convert_parameters(&self, parameters: &[Parameter]) -> Result<Vec<bc::Parameter>, MaxDiagnostic> {
        parameters
            .iter()
            .map(|p| {
                let typing = self.convert_optional_type(&p.type_ref)?;

                let parameter_kind = match &p.kind {
                    ParameterKind::Required => bc::ParamKind::Required(typing),
                    ParameterKind::Optional => bc::ParamKind::Optional(typing),
                    ParameterKind::Variadic => bc::ParamKind::Variadic(typing),
                };

                Ok(bc::Parameter { name: p.name.node.clone(), parameter_kind })
            })
            .collect()
    }

    /// Decides which argument goes to which parameter. Does NOT emit any bytecode.
    ///
    /// It works in three passes. The order is the whole trick! 
    ///
    /// **1. Named arguments claim their parameters first.** For each `name = value`, find
    /// the parameter called `name` and mark it taken. This is the "remove it from the
    /// temporary stack" idea: claimed parameters are simply not free any more,
    /// thus the unnamed arguments will skip over them.\
    /// **Errors**: unknown name, same name given twice, or the name is ambiguous
    /// (events: two handlers both have a param called `str`).
    ///
    /// **2. Unnamed arguments fill the leftover (free) parameters, left to right.**
    /// If the free parameters run out, the extras go to the variadic parameter (if there
    /// is one), otherwise *"too many arguments"*.
    ///
    /// **3. Check nothing required is missing.** A `Required` parameter with no argument
    /// is an error. An `Optional` one is fine (it becomes nil).\
    /// Note that Events skip this check, since every event payload may be omitted.
    ///
    /// ```text
    /// foo(name = "Max", false, 10)  with params (name, flag, count)
    ///   pass 1: name -> claims slot 0                       fixed = [Some(0), None, None]
    ///   pass 2: free slots are [1, 2]
    ///           false -> slot 1, 10 -> slot 2               fixed = [Some(0), Some(1), Some(2)]
    ///   pass 3: nothing missing, done
    ///
    /// foo(b = 2, 1)  with params (a, b)
    ///   pass 1: b -> claims slot 1                          fixed = [None, Some(0)]
    ///   pass 2: free slots are [0]; `1` -> slot 0           fixed = [Some(1), Some(0)]
    /// ```
    ///
    /// Note that unnamed arguments may come AFTER named ones. That is a deliberate Max
    /// rule (Python would forbid it).
    ///
    /// * `callee`: only used in error messages
    /// * `params`: the callee's parameters (variadic, if any, must be last)
    /// * `span`: where to point the "missing argument" error
    /// * `event`: true when binding an `emit`
    fn bind_arguments(&self, callee: &str, params: &[ParamSignature], args: &[Argument], span: Span, event: bool, allow_variadics: bool)
        -> Result<Binding, MaxDiagnostic>
    {
        // Step by Step! \\
        // First check if a variadic exists, then the *fixed* params if it does exist!
        let has_variadic = params.last().is_some_and(|p| matches!(p.kind, ParameterKind::Variadic));
        if has_variadic && !allow_variadics {
            return Err(error(span, "variadics-disabled", "Cannot use variadic parameters here"));
        }
        let fixed_params = if has_variadic { &params[..params.len() - 1] } else { params };
        
        // A seperate vec for fixed params that have a specified index in the param list
        let mut fixed: Vec<Option<usize>> = vec![None; fixed_params.len()];

        // 1. Named arguments claim their parameters
        for (i, arg) in args.iter().enumerate() {
            let Some(named) = &arg.name else { continue };

            // If more than one match is found, that's nonesensical!
            let matches = fixed_params.iter().filter(|p| p.name == named.node).count();
            if matches > 1 {
                return Err(error(named.span, "binded-parameter-handler-overload", format!(
                    "'{}' names a parameter in more than one handler of '{callee}', so it can't be used as a name", named.node)));
            }

            // Or just does not exist?
            let Some(slot) = fixed_params.iter().position(|p| p.name == named.node) else {
                return Err(error(named.span, "missing-binded-parameter", format!("'{callee}' has no parameter called '{}'", named.node)));
            };

            // Already?
            if fixed[slot].is_some() {
                return Err(error(named.span, "binding-parameter-overload", format!("Parameter '{}' was given more than once", named.node)));
            }
            // When all checks pass
            fixed[slot] = Some(i);
        }

        // 2. Unnamed arguments fill what is left, in order
        // Complex ahh. But filters any leftover fixed elements (have 'None' aka no name)
        let mut free = (0..fixed.len()).filter(
            |&s| fixed[s].is_none()
        ).collect::<Vec<_>>().into_iter();

        // For when a variadic exists, where all extra arguments fall through to
        let mut variadic = Vec::new();

        for (i, arg) in args.iter().enumerate() {
            if arg.name.is_some() { continue; }
            
            // Match against a free param when the arg has no name
            match free.next() {
                // Slot assigned
                Some(slot) => fixed[slot] = Some(i),
                // No slot, but it's variadic
                None if has_variadic => variadic.push(i),
                // Random bs go?
                None => return Err(error(arg.value.span, "task-parameter-overload", format!(
                                "'{callee}' takes at most {} argument(s), but more were given", fixed_params.len()))),
            }
        }

        // 3. Nothing required may be missing
        if !event /* Events ignore missing params anyway */ {
            for (slot, p) in fixed_params.iter().enumerate() {
                if fixed[slot].is_none() && matches!(p.kind, ParameterKind::Required) {
                    return Err(error(span, "missing-task-argument", format!("Missing argument for parameter '{}' of '{callee}'", p.name)));
                }
            }
        }

        Ok(Binding { fixed, variadic })
    }

    // ---------------------------- //
    // ----------- ENUMS ---------- //
    // ---------------------------- //

    fn find_enum_members<'s>(&'s self, name: &str) -> Option<&'s [EnumMember]> {
        if let Some(members) = self.local_enums.get(name) {
            return Some(members.as_slice());
        }

        for dependency in self.dependencies {
            if let Some(table) = self.ctx.symbols.get(dependency)
                && let Some(SymbolInfo::Enum(members)) = table.contents.get(name)
            {
                return Some(members.as_slice());
            }
        }

        None
    }

    /// A member's children: its inline sub members, or the members of the enum it references
    fn sub_members_of<'s>(&'s self, member: &'s EnumMember, span: Span) -> Result<&'s [EnumMember], MaxDiagnostic> {
        match &member.sub_enum_reference {
            Some(reference) => self.find_enum_members(reference).ok_or_else(|| {
                error(span, "missing-submembers", format!("Sub enum '{reference}' referenced by '{}' was not found", member.name))
            }),
            None => Ok(member.sub_members.as_slice()),
        }
    }

    /// Works out what an expression refers to inside the enum hierarchy, one step at a time
    fn resolve_enum_node<'s>(&'s self, expression: &'s Expression) -> Result<Option<EnumNode<'s>>, MaxDiagnostic> {
        match &expression.kind {
            // ItemType
            ExpressionKind::Variable(name) if !self.is_variable_declared(name) => Ok(self
                .find_enum_members(name)
                .map(|children| EnumNode { kind: NodeKind::Definition, path: name.clone(), children })),

            ExpressionKind::MemberAccess { target, member } => {
                // Module-qualified enum: map.Direction
                if let ExpressionKind::Variable(module) = &target.kind
                    && !self.is_variable_declared(module)
                    && let Some(table) = self.ctx.symbols.get(module)
                    && let Some(SymbolInfo::Enum(members)) = table.contents.get(member)
                {
                    return Ok(Some(EnumNode {
                        kind: NodeKind::Definition,
                        path: format!("{module}.{member}"),
                        children: members.as_slice(),
                    }));
                }

                // Otherwise resolve the parent, then take exactly ONE step into it
                let Some(parent) = self.resolve_enum_node(target)? else {
                    return Ok(None);
                };

                let child = parent
                    .children
                    .iter()
                    .find(|m| m.name == *member)
                    .ok_or_else(|| error(expression.span, "missing-submember", format!("'{member}' is not a member of '{}'", parent.path)))?;

                Ok(Some(EnumNode {
                    kind: NodeKind::Member,
                    path: format!("{}.{}", parent.path, child.name),
                    children: self.sub_members_of(child, expression.span)?,
                }))
            }

            _ => Ok(None),
        }
    }

    fn resolve_member_access_type(
        &self,
        target: &Expression,
        member: &str,
        span: Span,
    ) -> Result<bc::MemberAccessType, MaxDiagnostic> {
        // Take in steps, just don't trip!
        // 1. An enum definition or an enum member
        if let Some(node) = self.resolve_enum_node(target)? {
            if !node.children.iter().any(|m| m.name == member) {
                return Err(error(span, "missing-enum-member", format!("'{member}' is not a member of '{}'", node.path)));
            }
            return Ok(match node.kind {
                NodeKind::Definition => bc::MemberAccessType::Enum,
                NodeKind::Member => bc::MemberAccessType::EnumMember,
            });
        }

        // 2. Static model accessing
        if let Some(object) = self.resolve_object_node(target) {
            self.check_static_member(&object, member, MemberKind::Field, span)?;
            return Ok(bc::MemberAccessType::StaticObject)
        }

        // 3. Module access (map.Direction)
        if let ExpressionKind::Variable(name) = &target.kind
            && !self.is_variable_declared(name)
            && self.ctx.symbols.contains_key(name)
        {
            return Ok(bc::MemberAccessType::Module);
        }

        // A memory of our C# Compiler😭: a nested access whose first member is itself a module name
        if let ExpressionKind::MemberAccess { member: nested, .. } = &target.kind
            && self.ctx.symbols.contains_key(nested)
        {
            return Ok(bc::MemberAccessType::Module);
        }

        // 4. Anything else is an instance property
        Ok(bc::MemberAccessType::None)
    }

    fn compile_enum_definition(&mut self, definition: &EnumDefinition) -> Result<(), MaxDiagnostic> {
        if self.has_duplicate_naming(&definition.identifier) {
            return Err(error(
                definition.name_span, "duplicate-enum-name",
                format!("Duplicate naming found when attempting to create enum '{}'", definition.identifier),
            ));
        }

        // Nested enum definitions are not hoisted, so make sure they are known
        self.local_enums
            .entry(definition.identifier.clone())
            .or_insert_with(|| definition.members.clone());

        let members = self.build_members(&definition.identifier, &[], &definition.members)?;
        let structure = bc::EnumStructure { identifier: definition.identifier.clone(), enum_members: members };

        let mut_enums = Arc::make_mut(&mut self.enums);
        mut_enums.insert(definition.identifier.clone(), structure.clone());
        self.emit(bc::Op::DefineEnum(structure));
        Ok(())
    }

    /// Fills in each member's enum name and path, and checks for duplicates and missing sub enums
    fn build_members(
        &self,
        enum_name: &str,
        parent_path: &[String],
        members: &[EnumMember],
    ) -> Result<Vec<bc::EnumMember>, MaxDiagnostic> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut built = Vec::with_capacity(members.len());

        for member in members {
            if !seen.insert(member.name.as_str()) {
                return Err(error(
                    member.span, "duplicate-enum-member",
                    format!("Duplicate enum member found '{}' in '{enum_name}'", member.name),
                ));
            }

            if let Some(reference) = &member.sub_enum_reference
                && self.find_enum_members(reference).is_none()
            {
                return Err(error(member.span, "missing-subenum-reference",
                        format!("Sub enum '{reference}' was not found")));
            }

            let mut path = parent_path.to_vec();
            path.push(member.name.clone());

            built.push(bc::EnumMember {
                name: member.name.clone(),
                enumerable: member.enumerable,
                enum_name: enum_name.to_string(),
                sub_members: self.build_members(enum_name, &path, &member.sub_members)?,
                path,
                sub_enum_reference: member.sub_enum_reference.clone(),
            });
        }

        Ok(built)
    }

    // ---------------------------- //
    // -------- STATEMENTS -------- //
    // ---------------------------- //

    fn compile_statement(&mut self, statement: &Statement) -> Result<(), MaxDiagnostic> {
        match &statement.kind {
            // Only provides dependencies, nothing to emit
            StatementKind::Using(_) => {}

            StatementKind::Say(value) => {
                self.compile_expression(value)?;
                self.emit(bc::Op::Say);
            }

            StatementKind::Declaration(declaration) => self.compile_declaration(declaration)?,
            StatementKind::Assignment(assignment) => self.compile_assignment(assignment)?,
            StatementKind::If(if_statement) => self.compile_if(if_statement)?,
            StatementKind::While(while_statement) => self.compile_while(while_statement)?,
            StatementKind::For(for_statement) => self.compile_for(for_statement)?,
            StatementKind::Foreach(foreach_statement) => self.compile_foreach(foreach_statement)?,
            StatementKind::Match(match_statement) => self.compile_match(match_statement)?,

            // These don't need operands as they just signal the compiler to do something
            StatementKind::Break => self.compile_break(statement.span)?,
            StatementKind::Skip => self.compile_skip(statement.span)?,

            StatementKind::Expression(expression) => {
                self.compile_expression(expression)?;
                self.emit(bc::Op::Pop);
            }

            StatementKind::Return(value) => self.compile_return(value.as_ref(), statement.span)?,
            StatementKind::Yield(value) => self.compile_yield(value.as_ref())?,
            StatementKind::Emit(event) => self.compile_emit(event)?,

            StatementKind::TaskDefinition(task) => self.compile_task_definition(task)?,

            // These are compiled before everything else, they should be ignored
            StatementKind::Model(_) => return Err(error(statement.span, "not-top-level-model", "Models can only be declared at the top level of a file")),
            StatementKind::Enum(_) => return Err(error(statement.span, "not-top-level-enum", "Enums can only be declared at the top level of a file")),
        }
        Ok(())
    }

    fn compile_declaration(&mut self, declaration: &Declaration) -> Result<(), MaxDiagnostic> {
        if self.task_depth.len() == 1 && self.has_duplicate_naming(&declaration.name) {
            return Err(error(
                declaration.name_span, "declaration-duplicate",
                format!("A task or variable of the same name has already been defined: {}", declaration.name),
            ));
        }

        let lifecycle: &Option<Lifecycle> = &declaration.modifiers.lifecycle;
        let mutability: &Option<Mutability> = &declaration.modifiers.mutability;

        let (open, monic, fixed) = (
            lifecycle.is_some_and(|lc| lc == Lifecycle::Open),
            lifecycle.is_some_and(|lc| lc == Lifecycle::Monic),
            mutability.is_some_and(|mt| mt == Mutability::Fixed)
        );

        self.variable_scopes
            .last_mut()
            .expect("the compiler always has a scope")
            .insert(declaration.name.clone(), VarInfo { open, fixed, monic });

        if self.expression_monic_origin(&declaration.value).is_monic() {
            return Err(error(declaration.value.span, "monic-value-transfer", "A monic value cannot be transferred into a declaration"));
        }

        self.compile_expression(&declaration.value)?;

        let max_type = self.convert_optional_type(&declaration.type_ref)?;
        let resort_value = match &declaration.resortable {
            Some(expression) => Some(to_const(expression)?),
            None => None,
        };

        self.emit(bc::Op::Store(bc::StoreOperand {
            name: declaration.name.clone(),
            modifiers: convert_modifiers(&declaration.modifiers),
            max_type,
            resort_value,
        }));
        Ok(())
    }

    fn compile_assignment(&mut self, assignment: &Assignment) -> Result<(), MaxDiagnostic> {
        let operation = update_operation(assignment.op);

        if self.expression_monic_origin(&assignment.value).is_monic() {
            return Err(error(assignment.value.span, "monic-value-transfer", "A monic value cannot be transferred into an assignment"));
        }

        match &assignment.target.kind {
            ExpressionKind::Variable(name) => {
                if self.is_expression_of(&assignment.target, "fixed") {
                    return Err(error(assignment.target.span, "fixed-variable-mutation", "Cannot reassign a fixed variable"));
                }
                self.compile_expression(&assignment.value)?;
                self.emit(bc::Op::Update(bc::UpdateOperand { name: name.clone(), operation }));
            }

            // Member accessing `this`!
            ExpressionKind::MemberAccess { target, member }
            if matches!(target.kind, ExpressionKind::This) => {
                // Playing checkers, haha
                self.check_this(target.span)?;
                self.check_this_member(member, MemberKind::Field, target.span)?;
                
                self.compile_expression(&assignment.value)?;
                self.emit(bc::Op::Update(bc::UpdateOperand {
                    name: member.clone(),
                    operation
                }));
            }

            ExpressionKind::Index { target, index } => {
                if self.is_expression_of(target, "fixed") {
                    return Err(error(assignment.target.span, "fixed-variable-mutation", "Cannot modify an item through a fixed variable"));
                }
                self.compile_expression(target)?;
                self.compile_expression(index)?;
                self.compile_expression(&assignment.value)?;

                let target_name = Self::break_member_access(target).map(str::to_string);
                self.emit(bc::Op::UpdateIndex(bc::UpdateIndexOperand { operation, target_name }));
            }

            _ => return Err(error(assignment.target.span, "unassignable-expression", "This expression cannot be assigned to")),
        }
        Ok(())
    }

    fn compile_if(&mut self, statement: &IfStatement) -> Result<(), MaxDiagnostic> {
        // Jumps from the end of each branch to the end of the whole statement
        let mut end_jumps = Vec::new();
        // Jumps taken when the condition is a `possible`, which go to the `maybe` block
        let mut possible_jumps = Vec::new();

        for branch in std::iter::once(&statement.first).chain(statement.elifs.iter()) {
            self.compile_expression(&branch.condition)?;

            possible_jumps.push(self.emit_jump(bc::Op::JumpIfPossible));
            let false_jump = self.emit_jump(bc::Op::JumpIfFalse);

            self.compile_scoped_block(&branch.body)?;

            end_jumps.push(self.emit_jump(bc::Op::Jump));

            // The false jump lands here, on the next branch
            let next = self.ops.len();
            self.patch_jump(false_jump, next);
        }

        // else
        self.compile_scoped_block(&statement.else_block)?;

        let maybe_jump = self.emit_jump(bc::Op::Jump);
        let maybe_start = self.ops.len();

        // maybe
        self.compile_scoped_block(&statement.maybe_block)?;

        let end = self.ops.len();
        self.patch_jump(maybe_jump, end);
        for jump in end_jumps {
            self.patch_jump(jump, end);
        }
        for jump in possible_jumps {
            self.patch_jump(jump, maybe_start);
        }
        Ok(())
    }

    fn compile_while(&mut self, statement: &WhileStatement) -> Result<(), MaxDiagnostic> {
        // The condition is re-checked at the start of every iteration
        let condition_start = self.ops.len();
        // We'll use this to determine the depth for breaking and skipping
        let depth = self.variable_scopes.len();

        self.compile_expression(&statement.condition)?;
        let exit_jump = self.emit_jump(bc::Op::JumpIfFalse);

        self.push_jump_context(JumpKind::Loop, depth, depth);
        self.compile_scoped_block(&statement.body)?;
        let context = self.pop_jump_context();

        self.emit(bc::Op::Jump(condition_start));

        let exit = self.ops.len();
        self.patch_jump(exit_jump, exit);
        // Patch these jumps. Telling each `break` and `skip` where to jump
        for jump in context.break_jumps { self.patch_jump(jump, exit); } // To the end
        for jump in context.skip_jumps { self.patch_jump(jump, condition_start); } // To the beginning (of the condition)
        Ok(())
    }

    fn compile_for(&mut self, statement: &ForStatement) -> Result<(), MaxDiagnostic> {
        // This will be a very linear set of instructions...
        let name = &statement.declaration.name;

        // The loop variable lives in a scope of its own
        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

        // Why after `enter_compiler_scope`? See later
        let loop_depth = self.variable_scopes.len();

        self.compile_declaration(&statement.declaration)?;

        let step_slot = self.ctx.alloc_slot();
        self.compile_expression(&statement.step)?;
        self.emit(bc::Op::TemporaryStore(step_slot));

        // A zero step can never make progress
        self.compile_temporary_comparison(step_slot, bc::CompareOp::NotEqual);
        let zero_step_exit = self.emit_jump(bc::Op::JumpIfFalse);
        let past_zero_check = self.emit_jump(bc::Op::Jump);

        let zero_step_error = self.ops.len();
        self.emit(bc::Op::ExitScope);
        self.emit(bc::Op::Throw(String::from("A for loops step amount cannot be 0")));
        self.patch_jump(zero_step_exit, zero_step_error);

        let condition_start = self.ops.len();
        self.patch_jump(past_zero_check, condition_start);

        // A positive step counts up, so keep going while variable <= limit
        self.compile_temporary_comparison(step_slot, bc::CompareOp::Greater);
        let negative_branch = self.emit_jump(bc::Op::JumpIfFalse);

        self.compile_for_bound_check(name, &statement.limit, bc::CompareOp::LesserEqual)?;
        let positive_exit = self.emit_jump(bc::Op::JumpIfFalse);
        let body_jump = self.emit_jump(bc::Op::Jump);

        // A negative step counts down, so keep going while variable >= limit
        let negative_start = self.ops.len();
        self.patch_jump(negative_branch, negative_start);

        self.compile_for_bound_check(name, &statement.limit, bc::CompareOp::GreaterEqual)?;
        let negative_exit = self.emit_jump(bc::Op::JumpIfFalse);

        let body_start = self.ops.len();
        self.patch_jump(body_jump, body_start);

        // Our loop depth allows the compiler to exit the compiler scope *before* having to exit via a break
        self.push_jump_context(JumpKind::Loop, loop_depth, loop_depth);
        self.compile_scoped_block(&statement.body)?;
        let context = self.pop_jump_context();
        let increment_start = self.ops.len();

        // variable = variable + step
        self.emit(bc::Op::Load(name.clone()));
        self.emit(bc::Op::TemporaryLoad(step_slot));
        self.emit(bc::Op::Add);
        self.emit(bc::Op::Update(bc::UpdateOperand { name: name.clone(), operation: bc::UpdateOperation::Assign }));

        self.emit(bc::Op::Jump(condition_start));

        let exit = self.ops.len();
        self.emit(bc::Op::ExitScope);
        self.exit_compiler_scope();

        self.patch_jump(positive_exit, exit);
        self.patch_jump(negative_exit, exit);
        for jump in context.break_jumps { self.patch_jump(jump, exit); }
        for jump in context.skip_jumps { self.patch_jump(jump, increment_start); }
        Ok(())
    }

    /// Pushes `slot <op> 0`, which decides how a `for` loop counts
    fn compile_temporary_comparison(&mut self, slot: u64, op: bc::CompareOp) {
        self.emit(bc::Op::TemporaryLoad(slot));
        self.emit_constant(bc::ConstValue::Integer(0));
        self.emit(bc::Op::Compare(op));
    }

    /// Pushes `variable <op> limit`
    fn compile_for_bound_check(&mut self, name: &str, limit: &Expression, op: bc::CompareOp) -> Result<(), MaxDiagnostic> {
        self.emit(bc::Op::Load(name.to_string()));
        self.compile_expression(limit)?;
        self.emit(bc::Op::Compare(op));
        Ok(())
    }

    fn compile_foreach(&mut self, statement: &ForeachStatement) -> Result<(), MaxDiagnostic> {
        // Evaluate the collection once and keep it in a temporary slot
        let collection_slot = self.ctx.alloc_slot();
        self.compile_expression(&statement.iterable)?;
        self.emit(bc::Op::TemporaryStore(collection_slot));

        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

        let loop_depth = self.variable_scopes.len();

        // The loop variables start as nil
        for name in &statement.variables {
            self.emit_constant(bc::ConstValue::Nil);
            self.emit(bc::Op::Store(bc::StoreOperand {
                name: name.clone(),
                modifiers: bc::Modifiers::default(),
                max_type: None,
                resort_value: None,
            }));
        }

        // One slot tracks the position, even with several variables
        let index_slot = self.ctx.alloc_slot();
        self.emit_constant(bc::ConstValue::Integer(0));
        self.emit(bc::Op::TemporaryStore(index_slot));

        let loop_start = self.ops.len();

        self.emit(bc::Op::ForeachNext(bc::ForeachOperand {
            collection_slot,
            index_slot,
            declarations: statement.variables.clone(),
            variable_type: None,
        }));
        let exit_jump = self.emit_jump(bc::Op::JumpIfFalse);

        self.push_jump_context(JumpKind::Loop, loop_depth, loop_depth);
        // Each iteration gets its own scope, so `declare` inside the body works every time
        self.compile_scoped_block(&statement.body)?;
        let context = self.pop_jump_context();
        self.emit(bc::Op::Jump(loop_start));

        let exit = self.ops.len();
        self.emit(bc::Op::ExitScope);
        self.exit_compiler_scope();
        self.patch_jump(exit_jump, exit);

        for jump in context.break_jumps { self.patch_jump(jump, exit); }
        for jump in context.skip_jumps { self.patch_jump(jump, loop_start); }
        Ok(())
    }

    fn compile_match(&mut self, statement: &MatchStatement) -> Result<(), MaxDiagnostic> {
        /*
            Our match design is very similar to C#:
             - Evaluate the subject once, so `match next() then` doesn't call it once per case (aka, one evaluated value to match against)
             - Check through cases. Once a case is matched, go through it's body
             - If the case has NO break, fall through to next cases body, and the next if there is further no break
             - If a break is reached, end the matching or fallthroughs
             - If no match is made and an 'else' exists, go through that instead
             * The else is run too if no break from the other cases are hit!
        */
        let slot = self.ctx.alloc_slot();
        self.compile_expression(&statement.condition)?;
        self.emit(bc::Op::TemporaryStore(slot));

        let depth = self.variable_scopes.len();
        // `Match` JumpKind specifically!
        self.push_jump_context(JumpKind::Match, depth, depth);

        // 1. Tests: a match jumps to its body
        let mut body_jumps = Vec::new();
        for case in &statement.cases {
            self.emit(bc::Op::TemporaryLoad(slot));
            self.compile_expression(&case.condition)?;
            self.emit(bc::Op::Compare(bc::CompareOp::Equal));

            let next_test = self.emit_jump(bc::Op::JumpIfFalse);
            body_jumps.push(self.emit_jump(bc::Op::Jump));

            let here = self.ops.len();
            self.patch_jump(next_test, here);
        }
        // Nothing matched: step over every case body
        let to_default = self.emit_jump(bc::Op::Jump);

        // 2. Bodies, in order. With no jump between them, a body without `break` runs into the next
        for (case, jump) in statement.cases.iter().zip(body_jumps) {
            let start = self.ops.len();
            self.patch_jump(jump, start);
            self.compile_scoped_block(&case.body)?;
        }

        // 3. The default 'else'
        let default_start = self.ops.len();
        self.patch_jump(to_default, default_start);
        if let Some(default) = &statement.default {
            self.compile_scoped_block(default)?;
        }

        let context = self.pop_jump_context();
        let end = self.ops.len();
        // Patch all breaks. We don't do skips in matches, boy
        for jump in context.break_jumps { self.patch_jump(jump, end); }
        Ok(())
    }

    fn compile_return(&mut self, value: Option<&Expression>, span: Span) -> Result<(), MaxDiagnostic> {
        if self.task_depth.len() == 1 {
            return Err(error(span, "return-outside-task", "Return statements can only be used inside a task."));
        }
        if let Some(bool) = self.task_depth.last() && bool == &false {
            return Err(error(span, "return-in-unreturnable-task","Return statements can only be used inside a returnable task."));
        }

        match value {
            None => self.emit(bc::Op::Return),
            Some(expression) => {
                self.compile_expression(expression)?;
                self.emit(bc::Op::ReturnValue);
            }
        }
        Ok(())
    }

    fn compile_yield(&mut self, value: Option<&Expression>) -> Result<(), MaxDiagnostic> {
        match value {
            Some(expression) => self.compile_expression(expression)?,
            None => self.emit_constant(bc::ConstValue::Nil),
        }
        self.emit(bc::Op::Yield);
        Ok(())
    }

    fn compile_emit(&mut self, call: &Expression) -> Result<(), MaxDiagnostic> {
        let ExpressionKind::Callable { callee, arguments } = &call.kind else {
            return Err(error(call.span, "emit-without-event", "'emit' must be followed by an event call, e.g. emit name(payload)"));
        };

        // `name` in this file, or `module.name` from a `using` module
        // We always have to do this...
        let (target, name, arity) = match &callee.kind {
            ExpressionKind::Variable(name) => match self.local_events.get(name) {
                Some(&arity) => (None, name, arity),
                // Not a found local event? What a scam!
                None => return Err(error(callee.span, "unknown-event", format!("Unknown event '{name}'"))),
            },

            ExpressionKind::MemberAccess { target, member } => {
                // Yada yada...
                let ExpressionKind::Variable(module) = &target.kind else {
                    return Err(error(callee.span, "emit-without-event", "Expected an event name or module.event after 'emit'"));
                };
                if !self.dependencies.contains(module) {
                    return Err(error(callee.span, "missing-module-event", format!("Module '{module}' must be imported with 'using' to emit its events")));
                }
                match self.ctx.symbols.get(module).and_then(|t| t.contents.get(member)) {
                    Some(SymbolInfo::Event { parameters }) => (Some(module.clone()), member, parameters.len()),

                    _ => return Err(error(callee.span, "missing-module-event", format!("Module '{module}' has no event '{member}'"))),
                }
            }

            _ => return Err(error(callee.span, "missing-event-name", "Expected an event name after 'emit'")),
        };

        // Yippee doodle!
        if arguments.len() > arity {
            return Err(error(callee.span, "missing-event-payload", format!(
                "Event '{name}' takes at most {arity} payload(s) across all its handlers, but {} were given",
                arguments.len()
            )));
        }

        let signature = self.lookup_signature(name);

        self.compile_call_arguments(name, signature, arguments, callee.span, true)?;
        for _ in arguments.len()..arity {
            self.emit_constant(bc::ConstValue::Nil); // Omitted payloads become nil, quite indeed
        }

        // Emitting a task is otherwise really similar to a task call!
        self.emit(bc::Op::Call(bc::TaskCallOperand {
            target,
            name: format!("{name}#emit"),
            argument_count: arity as u64,
            is_member_access: false,
        }));
        self.emit(bc::Op::Pop);
        Ok(())
    }

    // ---------------------------- //
    // ---------- TASKS ----------- //
    // ---------------------------- //

    fn emit_event_dispatcher(&mut self, event: &str, handlers: &[EventHandler]) {
        let total: usize = handlers.iter().map(|h| h.parameter_count).sum();
        let skip_body = self.emit_jump(bc::Op::Jump);
        let entry_point = self.ops.len() as u64;

        let mut_tasks = Arc::make_mut(&mut self.tasks);

        mut_tasks.insert(format!("{event}#emit"), bc::TaskInfo {
            entry_point,
            parameters: (0..total)
                // Params are ALWAYS Optional in an Event!
                .map(|i| bc::Parameter {
                    name: format!("#{i}"),
                    parameter_kind: max_bytecode::ParamKind::Optional(None)
                })
                .collect(),
            return_type: None,
        });

        let mut offset = 0;
        for handler in handlers {
            for i in offset..offset + handler.parameter_count {
                self.emit(bc::Op::Load(format!("#{i}")));
            }
            self.emit(bc::Op::Call(bc::TaskCallOperand {
                target: None,
                name: handler.task_name.clone(),
                argument_count: handler.parameter_count as u64,
                is_member_access: false,
            }));
            self.emit(bc::Op::Pop); // a handler's Return pushes nil
            offset += handler.parameter_count;
        }
        self.emit(bc::Op::Return);

        let end = self.ops.len();
        self.patch_jump(skip_body, end);
    }

    fn compile_task_definition(&mut self, task: &TaskDefinition) -> Result<(), MaxDiagnostic> {
        // Just a sneaky peek
        let is_event = task.kind == TaskKind::Event;

        if is_event {
            // Only top level
            if self.task_depth.len() != 1 {
                return Err(error(task.name_span, "non-top-level-event", "Events can only be defined at the top level of a file"));
            }
            // No return types are wanted
            if task.return_type.is_some() {
                return Err(error(task.name_span, "event-return-type", "Events cannot return a value, so they cannot declare a return type"));
            }
            // Not an event name
            if self.non_event_name(&task.name) {
                return Err(error(task.name_span, "duplicate-task-name", format!("A task, variable or enum of the same name has already been defined: {}", task.name)));
            }

            // Check for variadic params in an event, which are NOT allowed!
            if let Some(variadic) = task.parameters.iter().find(|p| p.kind == ParameterKind::Variadic) {
                return Err(error(
                    task.name_span,
                    "event-variadic-payload",
                    format!(
                        "Event handlers cannot have variadic parameters ('{}...'). \
                        Payloads are split between handlers by position, so use a list payload instead",
                        variadic.name.node
                    ),
                ));
            }
        }
        else if self.has_duplicate_naming(&task.name) {
            return Err(error(
                task.name_span,
                "duplicate-task-name",
                format!("A task or variable of the same name has already been defined: {}", task.name),
            ));
        }

        // A function keeps its own name. A handler gets `name#index`
        let task_key = if is_event {
            let handlers = self.events.entry(task.name.clone()).or_default();
            let key = format!("{}#{}", task.name, handlers.len());
            handlers.push(EventHandler { task_name: key.clone(), parameter_count: task.parameters.len() });
            key
        } else {
            task.name.clone()
        };

        let return_type = self.convert_optional_type(&task.return_type)?;

        // Jump over the body when execution reaches the definition itself
        let skip_body = self.emit_jump(bc::Op::Jump);

        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

        let entry_point = self.ops.len() as u64;

        let saved_contexts = std::mem::take(&mut self.jump_contexts);

        match task.kind {
            TaskKind::Function | TaskKind::Event => {
                let parameters = self.convert_parameters(&task.parameters)?;

                let mut_tasks = Arc::make_mut(&mut self.tasks);

                // Registered before the body, so the task can call itself
                mut_tasks.insert(
                    // For an event, this is instead a unique ID key!
                    task_key.clone(),
                    bc::TaskInfo { entry_point, parameters, return_type },
                );

                for parameter in &task.parameters {
                    self.variable_scopes
                        .last_mut()
                        .expect("the compiler always has a scope")
                        .insert(parameter.name.node.clone(), VarInfo::default());
                }

                if task.is_native {
                    let key = format!("{}.{}", self.module_name, task.name);
                    // Native task -> Must be in module tasks!
                    if !self.ctx.natives.module_tasks.contains(&key) {
                        return Err(error(task.name_span, "missing-native-task", format!("No native task '{key}' is registered")));
                    }
                }

                self.task_depth.push(true);
                let result = if task.is_native {
                    self.emit(bc::Op::CallNativeForward(bc::TaskCallOperand {
                        target: Some(self.module_name.to_string()),
                        name: task.name.clone(),
                        argument_count: 0, // This is decided at runtime
                        is_member_access: false,
                    }));
                    self.emit(bc::Op::ReturnValue);
                    Ok(())
                } else {
                    self.compile_statements(&task.body)
                };
                self.task_depth.pop();
                result?;

                self.emit(bc::Op::Return);
                self.emit(bc::Op::ExitScope);
                self.exit_compiler_scope();

                let end = self.ops.len();
                self.patch_jump(skip_body, end);
            }

            TaskKind::Coroutine => {
                let info = bc::CoroutineInfo { entry_point, return_type };
                let mut_coroutines = Arc::make_mut(&mut self.coroutines);
                mut_coroutines.insert(task.name.clone(), info.clone());

                self.task_depth.push(true);
                let result = self.compile_statements(&task.body);
                self.task_depth.pop();
                result?;

                self.emit(bc::Op::ExitScope);
                self.exit_compiler_scope();

                // A coroutine that runs to the end returns nil
                self.emit_constant(bc::ConstValue::Nil);
                self.emit(bc::Op::ReturnValue);

                let end = self.ops.len();
                self.patch_jump(skip_body, end);

                // A coroutine is also a variable holding an instance, so `.run()` and friends work on it
                self.emit(bc::Op::CreateCoroutine(info));
                self.emit(bc::Op::Store(bc::StoreOperand {
                    name: task.name.clone(),
                    modifiers: bc::Modifiers::default(),
                    max_type: Some(bc::MaxType::CoroutineType),
                    resort_value: None,
                }));
            }
        }

        // Reenter the old jump contexts, so a task definition doesn't just jump through its self to break out a loop it is in
        self.jump_contexts = saved_contexts;
        Ok(())
    }

    /*
        COMPILING CALLING
    */

    /// Determines whether to compile an object construction, or a task/method
    /// invocation based on the `callee` given, using `resolve_object_node` to check.
    /// Arguments are passed into whatever will be compiled.
    fn determine_caller(&mut self, callee: &Expression, arguments: &[Argument]) -> Result<(), MaxDiagnostic> {
        if self.resolve_object_node(callee).is_some() {
            self.compile_object_construction(callee, arguments)
        } else {
            self.compile_task_call(callee, arguments)
        }
    }

    fn compile_object_construction(&mut self, callee: &Expression, arguments: &[Argument]) -> Result<(), MaxDiagnostic> {
        let Some(object) = self.resolve_object_node(callee) else {
            return Err(error(callee.span, "unknown-object", "Expected an object to construct"));
        };

        // The only things an object can be contained in is a module
        // `other.Player(..)` names its module. A bare `Player(..)` is local unless a `using` module supplied it
        let module = match &callee.kind {
            ExpressionKind::MemberAccess { target, .. } => match &target.kind {
                ExpressionKind::Variable(module) => Some(module.clone()),
                _ => None,
            },
            ExpressionKind::Variable(name) => self.object_module(name),
            _ => None,
        };

        // Yep...
        self.reject_monic_arguments(arguments)?;
        // `false` = no variadics, since it's too fiddly with objects
        let count = self.compile_call_arguments(&object.name, Some(object.parameters.clone()), arguments, callee.span, false)?;
        self.emit(bc::Op::Construct(bc::ConstructOperand { module, object: object.name.clone(), argument_count: count }));
        Ok(())
    }

    /// The module a bare object name comes from: `None` when it is defined **in** this file.
    /// Call this to get the module name of an object accessed **outside** the current file.
    fn object_module(&self, name: &str) -> Option<String> {
        if self.local_objects.contains_key(name) { return None; }
        self.dependencies.iter().find(|d| matches!(
            self.ctx.symbols.get(*d).and_then(|t| t.contents.get(name)),
            Some(SymbolInfo::Object(_))
        )).cloned()
    }

    fn compile_task_call(&mut self, callee: &Expression, arguments: &[Argument]) -> Result<(), MaxDiagnostic> {
        // Inside an object, static tasks are compiled with the object name, not `this`
        if let ExpressionKind::Variable(name) = &callee.kind
            && let Some(context) = &self.model_context
            && !self.is_variable_declared(name)
        {
            if let Some(member) = self.object_member(name) {
                if member.kind == MemberKind::Task && member.is_static {
                    let target = Expression::new(ExpressionKind::Variable(context.name.clone()), callee.span);
                    let rewritten = Expression::new(
                        ExpressionKind::MemberAccess { target: Box::new(target), member: name.clone() }, callee.span);
                    return self.compile_task_call(&rewritten, arguments);
                }

                // If not static, then why this-shaped?
                if member.kind == MemberKind::Task {
                    return Err(error(callee.span, "instance-task-needs-this",
                        format!("Call instance tasks through 'this': this.{name}(..)")));
                }
            }
        }

        if let ExpressionKind::MemberAccess { target, member } = &callee.kind {
            let is_fixed = self.is_expression_of(target, "fixed");

            match &target.kind {
                ExpressionKind::Variable(target_name) => {
                    // A variable of the same name hides any module
                    let shadowed = self.is_variable_declared(target_name);

                    // Imported module task: other.helper(x)
                    // IF not shadowed and the module exists!
                    if !shadowed
                        && let Some(table) = self.ctx.symbols.get(target_name)
                        && table.contents.contains_key(member)
                    {
                        // Get our signature and count here
                        let signature = match table.contents.get(member) {
                            Some(SymbolInfo::Task(t)) => Some(t.clone()),
                            _ => None
                        };
                        if signature.is_some() {
                            self.reject_monic_arguments(arguments)?;
                        }
                        let count = self.compile_call_arguments(member, signature, arguments, callee.span, true)?;

                        self.emit(bc::Op::Call(bc::TaskCallOperand {
                            target: Some(target_name.clone()),
                            name: member.clone(),
                            argument_count: count,
                            is_member_access: false,
                        }));
                        return Ok(());
                    }
                }

                _ => (),
            }

            // Otherwise, attempt to find if the target is an object!
            if let Some(object) = self.resolve_object_node(target) {
                // Of course, this must be a task member!
                self.check_static_member(&object, member, MemberKind::Task, callee.span)?;
            }

            if matches!(target.kind, ExpressionKind::This) {
                self.check_this(target.span)?;
                self.check_this_member(member, MemberKind::Task, callee.span)?;
            }

            if is_fixed && self.ctx.natives.transmutation_tasks.contains(member) {
                return Err(error(target.span, "fixed-variable-mutation", "Cannot call a transmutational task through a fixed variable"));
            }

            // Instance method: the target is pushed first, as `self`
            // Names can never be known like this
            self.compile_expression(target)?;
            let count = self.compile_call_arguments(member, None, arguments, callee.span, true)?;
            self.emit(bc::Op::CallNative(bc::TaskCallOperand {
                target: None,
                name: member.clone(),
                argument_count: count + 1, // + 'self'
                is_member_access: true,
            }));
            return Ok(());
        }

        // A plain call by name
        let signature = match &callee.kind {
            ExpressionKind::Variable(v) => self.lookup_signature(v),
            _ => None
        };
        if signature.is_some() {
            // Reject them with a seperate list of steps
            self.reject_monic_arguments(arguments)?;
        }
        // A custom label must be made since the callee is not a member access. We expect it to be a variable name otherwise
        let label = if let ExpressionKind::Variable(n) = &callee.kind {
            n.as_str()
        } else { "self-task" };
        let count = self.compile_call_arguments(label, signature, arguments, callee.span, true)?;

        let operand = self.resolve_task_operand(callee, count)?;
        self.emit(bc::Op::Call(operand));
        Ok(())
    }

    /// Checks if a given name is a member of the current `model_context`.
    /// This searches for *any* kind of member, and returns `Some(MemberSignature)`
    /// belonging to the member if found, else `None`
    fn object_member(&self, name: &str) -> Option<&MemberSignature> {
        let context = self.model_context.as_ref()?;
        self.local_objects.get(&context.name)?.members.iter().find(|m| m.name == name)
    }

    /// Breaks down a member access, finding if the first target (second last member) is
    /// a valid variable binding; An `Expression::Variable`, then gets the name from the found variable.
    ///
    /// This works **recursively**, so a variable will eventually be found. If no variable was ever found,
    /// or the method found an expression that could not be broken down further, then `None` is returned.
    fn break_member_access(expression: &Expression) -> Option<&str> {
        match &expression.kind {
            ExpressionKind::MemberAccess { target, .. } => {
                Self::break_member_access(target)
            }

            ExpressionKind::Index { target, .. } => {
                Self::break_member_access(target)
            }

            ExpressionKind::Variable(var) => Some(var),

            _ => None
        }
    }

    /// A super of `break_member_access` that returns the finally found Variable Expression
    /// instead of returning only it's name. This again also returnings `None` if no variable
    /// was found
    fn get_from_member_access(expression: &Expression) -> Option<Expression> {
        match &expression.kind {
            ExpressionKind::MemberAccess { target, .. } => {
                Self::get_from_member_access(target)
            }

            ExpressionKind::Index { target, .. } => {
                Self::get_from_member_access(target)
            }

            // End at a variable
            ExpressionKind::Variable(_) => Some(expression.clone()),

            _ => None
        }
    }

    /**
    Finds the parameter list of a task called by plain name, e.g. `foo(...)`.
    Returns `None` when the compiler cannot know the parameters, which means named
    arguments are not allowed for this call (see `compile_call_arguments`).

    The lookup order mirrors `resolve_task_operand`, so both agree on what `foo` means:
    1. A declared **variable** wins. It is probably a lambda, and a variable can be
       reassigned, so its parameters are unknowable at compile time.
    2. A task defined in **this file** (collected in pass 1 of `compile`, so order does not matter).
    3. A task from a module this files imports with **`using`**.

    Natives like `input` and methods like `list.add` are not found here, on purpose.

    Returns a clone, not a reference: the caller needs `&mut self` right afterwards to
    emit bytecode, and a borrowed signature would block that.
    */
    fn lookup_signature(&self, name: &str) -> Option<Vec<ParamSignature>> {
        if self.is_variable_declared(name) { return None; } // a lambda in a variable
        if let Some(sig) = self.local_tasks.get(name) { return Some(sig.clone()); }
        
        for dependency in self.dependencies {
            if let Some(SymbolInfo::Task(sig)) = self.ctx.symbols.get(dependency).and_then(|t| t.contents.get(name)) {
                return Some(sig.clone());
            }
        }
        None
    }

    /**
    Emits the bytecode for a call's arguments using a [`Binding`], and returns how many
    values the call will receive (the `argument_count` for `Op::Call`).

    The VM's `Call` expects its arguments on the stack **in parameter order**, but the
    user may have written them in any order, and side effects must still happen **in
    the order written**. So this does it in three steps:

    **1. Push every argument in SOURCE order.** `foo(b = f(), g())` runs `f` first,
    then `g`, exactly as written.

    **2. Push a `nil` for every parameter nobody supplied that still needs a slot.**
    These go after the real arguments. A gap BEFORE a supplied parameter must become an
    explicit nil. Trailing unfilled optionals are simply dropped (the VM already fills
    those). Events set `event = true` and always fill every slot, because the event
    dispatcher takes the full list.

    **3. Emit `Op::Permute(order)` to reorder what is on the stack.**
    `order[k]` means "the value pushed at position `order[k]` becomes the k-th argument".
    The variadic arguments are appended at the end, in source order. If `order` is
    already 0, 1, 2, ... nothing needs reordering! So no instruction is emitted and a
    normal call costs nothing extra.

    ```text
    foo(b = 2, 1)  with params (a, b),  binding.fixed = [Some(1), Some(0)]
      step 1: push 2, push 1          stack: [2, 1]      (source order)
      step 2: no gaps
      step 3: order = [1, 0]          Permute -> stack: [1, 2]   a = 1, b = 2

    foo(c = 3, 1)  with params (a, b?, c)   (b is optional)
      step 1: push 3, push 1          stack: [3, 1]
      step 2: b has no argument but sits before c, so push nil -> stack: [3, 1, nil]
      step 3: order = [1, 2, 0]       Permute -> stack: [1, nil, 3]
    ```

    Why not temporaries (`TemporaryStore` / `TemporaryLoad`), you might say? Their slots are global, so
    a recursive call at the same call site would overwrite them half way through.
    `Permute` works on the stack, which is recursion-safe.
    */
    fn compile_bound_arguments(&mut self, binding: &Binding, args: &[Argument], event: bool) -> Result<u64, MaxDiagnostic> {
        // Source order, so side effects happen as written
        for arg in args { self.compile_expression(&arg.value)?; }

        // Trailing unfilled optionals are dropped (the VM fills them with nil). Events always fill every slot,
        // since the dispatcher takes the full list. Gaps before a filled param become explicit nils.
        let keep = if event {
            binding.fixed.len()
        } else {
            binding.fixed.iter().rposition(|s| s.is_some()).map_or(0, |p| p + 1)
        };

        // One is for how many are pushed, another is the order of the args (in indexes)
        let mut pushed = args.len();
        let mut order: Vec<usize> = Vec::new();

        for slot in &binding.fixed[..keep] {
            // If an index, the ordered args are pushed into, otherwise push AND add to pushed counter
            match slot {
                Some(index) => order.push(*index),
                None => { self.emit_constant(bc::ConstValue::Nil); order.push(pushed); pushed += 1; }
            }
        }
        // Extend by the variadic bound
        order.extend(binding.variadic.iter().copied());

        // Now we PERMUTE all the ordered params
        // Note that the ordered vec can contain NILS found from the slot matching above
        if order.iter().enumerate().any(|(k, &i)| k != i) {
            self.emit(bc::Op::Permute(order.clone()));
        }
        Ok(order.len() as u64) // the `argument_count` for the Call
    }

    /**
    The front door for compiling a call's arguments. Returns the `argument_count`.

    * **Signature known** (`Some`): bind with `bind_arguments`, emit with
      `compile_bound_arguments`. Even calls with no named arguments go through this, so
      arity mistakes ("too many arguments", "missing argument") become compile errors
      instead of VM panics.
    * **Signature unknown** (`None`): named arguments are rejected with a clear error,
      and plain positional arguments are compiled in order as before.

    When is the signature unknown? Instance methods (`list.add(...)`, because the
    receiver's type isn't known until runtime), native tasks (their parameters have no
    names yet) and lambdas stored in variables (the variable could be reassigned).

    `callee` is only used for error messages.
    */
    fn compile_call_arguments(&mut self, callee: &str, signature: Option<Vec<ParamSignature>>, args: &[Argument], span: Span, allow_variadics: bool)
        -> Result<u64, MaxDiagnostic>
    {
        match signature {
            // There are params named? Great!
            Some(params) => {
                let binding = self.bind_arguments(callee, &params, args, span, false, allow_variadics)?;
                self.compile_bound_arguments(&binding, args, false)
            }

            // There are no params named? Great!
            None => {
                // But... what if the args are an imposter?
                if let Some(named) = args.iter().find_map(|a| a.name.as_ref()) {
                    return Err(error(named.span, "unknown-parameter-name", format!(
                        "Named arguments can't be used here: the parameters of '{callee}' aren't known at compile time")));
                }
                // Crewmates win
                for arg in args { self.compile_expression(&arg.value)?; }
                Ok(args.len() as u64)
            }
        }
    }

    /// Decides whether a plain call is local or goes to an imported module
    fn resolve_task_operand(&self, callee: &Expression, argument_count: u64) -> Result<bc::TaskCallOperand, MaxDiagnostic> {
        let ExpressionKind::Variable(name) = &callee.kind else {
            return Err(error(callee.span, "unnamed-task", "Only named tasks can be called."));
        };

        let operand = |target: Option<String>| bc::TaskCallOperand {
            target,
            name: name.clone(),
            argument_count,
            is_member_access: false,
        };

        // A lambda stored in a variable, or a task defined in this file
        if self.is_variable_declared(name) || self.tasks.contains_key(name) || self.coroutines.contains_key(name) {
            return Ok(operand(None));
        }

        // A task from a `using` module
        for dependency in self.dependencies {
            if let Some(table) = self.ctx.symbols.get(dependency)
                && table.contents.contains_key(name)
            {
                return Ok(operand(Some(dependency.clone())));
            }
        }

        // Defined later in this file, or a native global such as `input`. The VM resolves it at run time
        Ok(operand(None))
    }

    // ---------------------------- //
    // ---- MODEL DEFINITIONS ----- //
    // ---------------------------- //

    /// Resolve if an object (local or in dependencies) has the given name,
    /// returning it's `ObjectSignature` if so
    fn resolve_object(&self, name: &str) -> Option<ObjectSignature> {
        if let Some(object) = self.local_objects.get(name) {
            return Some(object.clone());
        }        

        for dependency in self.dependencies {
            if let Some(table) = self.ctx.symbols.get(dependency)
                && let Some(SymbolInfo::Object(object)) = table.contents.get(name)
            {
                return Some(object.clone());
            }
        }

        None
    }

    /// This resolves an object signature from the given `Expression`. This is
    /// not to be confused with `resolve_object`, which this function calls upon
    /// in certain arms.
    /// 
    /// The Expression is matched with:
    /// - `ExpressionKind::Variable` if no variable of same name => `resolve_object`
    /// - `ExpressionKind::MemberAccess` => check if the TARGET kind is a variable,
    /// then return the `ObjectSignature` obtained from the it's `SymbolInfo`
    fn resolve_object_node(&self, expression: &Expression) -> Option<ObjectSignature> {
        match &expression.kind {
            ExpressionKind::Variable(name) if !self.is_variable_declared(name) => self.resolve_object(name),

            ExpressionKind::MemberAccess { target, member } => {
                // Break it down, Yeah! In reality, objects could never be indexed like:
                // object[1].getObject()
                // So we rely on if the target only is an object
                if let ExpressionKind::Variable(module) = &target.kind
                    && !self.is_variable_declared(module)
                    && let Some(table) = self.ctx.symbols.get(module)
                    && let Some(SymbolInfo::Object(object)) = table.contents.get(member)
                {
                    return Some(object.clone());
                }
                // Otherwise nothin...
                None
            }

            _ => None,
        }
    }

    /// Checks if a member is a static member of the `ObjectSignature` given. The `kind` is
    /// used to force the member to one `MemberKind` (returning a diagnostic otherwise).
    fn check_static_member(&self, object: &ObjectSignature, member: &str, kind: MemberKind, span: Span) -> Result<(), MaxDiagnostic>
    {
        let Some(found) = object.members.iter().find(|m| m.name == member) else {
            return Err(error(span, "unknown-object-member",
                format!("'{member}' is not a member of '{}'", object.name)));
        };
        if found.kind != kind {
            return Err(error(span, "wrong-member-kind",
                format!("'{member}' is a {} member, not a {}",
                    if found.kind == MemberKind::Field { "field" } else { "task" },
                    if kind == MemberKind::Field { "field" } else { "task" })));
        }
        if !found.is_static {
            return Err(error(span, "instance-member-via-object",
                format!("'{member}' is an instance member; use it on an instance of '{}'", object.name)));
        }
        let inside_object = self.model_context.as_ref().is_some_and(|c| c.name == object.name);
        if found.visibility == Visibility::Private && !inside_object {
            return Err(error(span, "private-member", format!("'{member}' is private to '{}'", object.name)));
        }
        Ok(())
    }

    /// This function only first matches the `ModelDefinition` kind to call the appropriate compiler method
    fn compile_model_definition(&mut self, model: &ModelDefinition) -> Result<(), MaxDiagnostic> {
        match model {
            ModelDefinition::Object(object) => self.compile_object_definition(object)
        }
    }
        
    fn compile_object_definition(&mut self, object: &ObjectDefinition) -> Result<(), MaxDiagnostic> {
        let scope_count = self.variable_scopes.len();
        let result = self.compile_object_inner(object);
        // A failed object must not leak its scopes or context into later statements
        self.variable_scopes.truncate(scope_count);
        self.model_context = None;
        result
    }

    /// Works on compiling the inner-fields and methods of an object. This calls
    /// upon other methods specific to models too, such as `compile_function_body`
    fn compile_object_inner(&mut self, object: &ObjectDefinition) -> Result<(), MaxDiagnostic> {
        if self.has_duplicate_naming(&object.name.node) {
            return Err(error(object.name.span, "duplicate-object",
                format!("Object name has already been taken elsewhere: '{}'", object.name.node)));
        }

        // No variadic params
        if let Some(variadic) = object.parameters.iter().find(|p| p.kind == ParameterKind::Variadic) {
            return Err(error(variadic.name.span, "variadic-object-parameter",
                "Object construction parameters cannot be variadic"));
        }

        // One namespace for fields and tasks. We compare names, not whole members
        let mut seen: HashSet<&str> = HashSet::new();
        let mut fields: Vec<&FieldMember> = Vec::new();
        let mut tasks: Vec<&TaskMember> = Vec::new();
        for member in &object.members {
            match member {
                ModelMember::Field(f) => {
                    // Yuk Yuk
                    if !seen.insert(f.declaration.name.as_str()) {
                        return Err(f.declaration.name_span.error("duplicate-object-member",
                            format!("Another object member of the same name is taken: '{}'", f.declaration.name)));
                    }
                    fields.push(f);
                }
                ModelMember::Task(t) => {
                    if !seen.insert(t.definition.name.as_str()) {
                        return Err(t.definition.name_span.error("duplicate-object-member",
                            format!("Another object member of the same name is taken: '{}'", t.definition.name)));
                    }
                    // Functions only, *for now*. This also rejects events and coroutines
                    if t.definition.kind != TaskKind::Function || t.definition.is_native {
                        return Err(t.definition.name_span.error("unsupported-task-member",
                            format!("Task member '{}' is not supported yet", t.definition.name)));
                    }
                    tasks.push(t);
                }
            }
        }

        // Instance fields mean ALL fields, including statics
        let instance_fields: HashSet<String> = fields.iter().map(|f| f.declaration.name.clone()).collect();
        // Filter out the statics
        let static_fields: HashSet<String> = fields.iter()
            .filter(|f| f.attributes.is_static).map(|f| f.declaration.name.clone()).collect();

        self.model_context = Some(ModelContext {
            name: object.name.node.clone(),
            in_static_task: false,
            in_instance_task: false
        });

        // This will be a single jump over, like, everything
        let skip = self.emit_jump(bc::Op::Jump);

        // ---- Initialiser ---- \\
        let initialiser_entry = self.ops.len() as u64;
        // Scope for the construction params
        self.enter_compiler_scope();
        for parameter in &object.parameters {
            self.variable_scopes.last_mut().expect("the compiler always has a scope")
                .insert(parameter.name.node.clone(), VarInfo::default());
        }
        self.enter_compiler_scope();

        // Compile all fields now
        let mut field_infos: Vec<FieldInfo> = Vec::new();
        for field in &fields {
            field_infos.push(self.from_field_member(field)?);
            // Static fields live in the definition (static_value), not in each instance
            if !field.attributes.is_static {
                self.compile_declaration(&field.declaration)?;
            }
        }
        self.emit(bc::Op::FinishConstruct);
        self.exit_compiler_scope();
        self.exit_compiler_scope();

        // ---- Methods ---- \\
        let mut method_infos: Vec<MethodInfo> = Vec::new();
        for task in &tasks {
            let is_static = task.attributes.is_static;
            // is_static is true, and by default so is in_instance!
            if let Some(context) = self.model_context.as_mut() { context.in_static_task = is_static; context.in_instance_task = true }

            // Instance methods see every field, static methods only the static ones
            self.enter_compiler_scope();
            let visible = if is_static { &static_fields } else { &instance_fields };
            for name in visible {
                self.variable_scopes.last_mut().expect("the compiler always has a scope")
                    .insert(name.clone(), VarInfo::default());
            }

            // nothing emitted before this: no skip jump, no EnterScope.
            // Therefore it is correct to start here
            let entry_point = self.ops.len() as u64;
            method_infos.push(self.from_task_member(task, entry_point)?);
            self.compile_function_body(&task.definition)?;
            self.exit_compiler_scope();
        }

        self.patch_jump(skip, self.ops.len());

        let parameters = self.convert_parameters(&object.parameters)?;

        let new_object = bc::ObjectStructure {
            module_name: self.module_name.to_string(),
            identifier: object.name.node.clone(),
            initialiser_entry,
            parameters,
            fields: field_infos,
            methods: method_infos,
        };

        // Insert into the compiler objects, of course!
        Arc::make_mut(&mut self.objects).insert(
            object.name.node.clone(),
            new_object.clone(),
        );
        self.emit(bc::Op::DefineObject(new_object));
        Ok(())
    }

    /// Emits the Parameters + body + Return. The object owns the skip jump and the entry point,
    /// and nothing is registered in `self.tasks` (As this is for an object, not the global tasks).
    fn compile_function_body(&mut self, task: &TaskDefinition) -> Result<(), MaxDiagnostic> {
        self.enter_compiler_scope();
        for parameter in &task.parameters {
            self.variable_scopes.last_mut().expect("the compiler always has a scope")
                .insert(parameter.name.node.clone(), VarInfo::default());
        }

        // 'true' for now as events are not inside objects currently
        self.task_depth.push(true);
        let result = self.compile_statements(&task.body);
        self.task_depth.pop();
        result?;

        self.emit(bc::Op::Return);
        self.exit_compiler_scope();
        Ok(())
    }

    /// Converts a `FieldMember` into a `FieldInfo`. All values are sourced from the `FieldMember`
    /// itself, with help of the Compiler's methods.
    /// 
    /// The `max_bytecode::Visibility::Protected` is **not** supported yet
    fn from_field_member(&self, member: &FieldMember) -> Result<FieldInfo, MaxDiagnostic> {
        let visibility = convert_visibility_to_bytecode(&member.attributes.visibility);

        let modifiers: max_bytecode::Modifiers = convert_modifiers(&member.declaration.modifiers);
        let max_type = self.convert_optional_type(&member.declaration.type_ref)?;

        Ok(FieldInfo {
            name: member.to_name(),
            visibility,
            is_static: member.attributes.is_static,
            modifiers,
            max_type,
            static_value: if member.attributes.is_static {
                Some(to_const(&member.declaration.value)?) } else { None }
        })
    }

    /// Converts a `TaskMember` into a `MethodInfo`. The entry point *u64* value must provided as well
    /// for the task member.
    /// 
    /// Only the `max_bytecode::MemberTask::Function` is supported currently
    fn from_task_member(&self, task: &TaskMember, entry_point: u64) -> Result<MethodInfo, MaxDiagnostic> {
        let visibility = convert_visibility_to_bytecode(&task.attributes.visibility);

        let member_task: max_bytecode::MemberTask = match &task.definition.kind {
            TaskKind::Function => max_bytecode::MemberTask::Function(max_bytecode::TaskInfo {
                entry_point,
                parameters: self.convert_parameters(&task.definition.parameters)?,
                return_type: self.convert_optional_type(&task.definition.return_type)?
            }),

            _ => return Err(task.definition.name_span.error("unsupported-task-member", format!("Task member '{}' is not supported yet", task.definition.kind)))
        };

        Ok(MethodInfo {
            name: task.to_name(),
            visibility,
            is_static: task.attributes.is_static,
            task: member_task
        })
    }

    // ---------------------------- //
    // -------- EXPRESSIONS ------- //
    // ---------------------------- //

    fn compile_expression(&mut self, expression: &Expression) -> Result<(), MaxDiagnostic> {
        match &expression.kind {
            ExpressionKind::Integer(value) => self.emit_constant(bc::ConstValue::Integer(*value)),
            ExpressionKind::Float(value) => self.emit_constant(bc::ConstValue::Float(*value)),
            ExpressionKind::Str(value) => self.emit_constant(bc::ConstValue::Str(value.clone())),
            ExpressionKind::Boolean(value) => self.emit_constant(bc::ConstValue::Boolean(*value)),
            ExpressionKind::Possible => self.emit_constant(bc::ConstValue::Possible),
            ExpressionKind::Nil => self.emit_constant(bc::ConstValue::Nil),

            ExpressionKind::List(items) => {
                for item in items {
                    self.compile_expression(item)?;
                }
                self.emit(bc::Op::MakeList(items.len() as u32));
            }

            ExpressionKind::Dictionary(entries) => {
                for (key, value) in entries {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.emit(bc::Op::MakeDictionary(entries.len() as u32));
            }

            ExpressionKind::Variable(name) => self.emit(bc::Op::Load(name.clone())),

            ExpressionKind::Binary { left, op, right } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                self.emit(match op {
                    BinaryOp::Add => bc::Op::Add,
                    BinaryOp::Subtract => bc::Op::Subtract,
                    BinaryOp::Multiply => bc::Op::Multiply,
                    BinaryOp::Divide => bc::Op::Divide,
                    BinaryOp::Modulus => bc::Op::Modulus,
                });
            }

            ExpressionKind::Comparison { left, op, right } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                self.emit(bc::Op::Compare(compare_op(*op)));
            }

            // The C# compiler had no support for these either, and the VM has no instruction for them
            ExpressionKind::Logical { left, op, right } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                self.emit(match op {
                    LogicalOp::And => bc::Op::And,
                    LogicalOp::Or => bc::Op::Or
                });
            }

            ExpressionKind::Unary { op, operand } => {
                self.compile_expression(operand)?;
                self.emit(match op {
                    UnaryOp::Not => bc::Op::Not,
                    UnaryOp::Negate => bc::Op::Negate,
                });
            }

            ExpressionKind::Index { target, index } => {
                self.compile_expression(target)?;
                self.compile_expression(index)?;
                self.emit(bc::Op::Index);
            }

            ExpressionKind::MemberAccess { target, member } => {
                self.compile_member_access(target, member, expression.span)?
            }

            ExpressionKind::Callable { callee, arguments } => self.determine_caller(callee, arguments)?,
            ExpressionKind::This => self.check_this(expression.span)?,

            ExpressionKind::Lambda(lambda) => self.compile_lambda(lambda, expression.span)?,
        }
        Ok(())
    }

    fn compile_member_access(&mut self, target: &Expression, member: &str, span: Span) -> Result<(), MaxDiagnostic> {
        let access_type = self.resolve_member_access_type(target, member, span)?;

        if matches!(target.kind, ExpressionKind::This) {
            self.check_this(span)?;
            // `this` members specifically
            self.check_this_member(member,MemberKind::Field, span)?;
            self.emit(bc::Op::Load(member.to_string()));
            return Ok(());
        }

        if let ExpressionKind::Variable(name) = &target.kind
            && !self.is_variable_declared(name)
            && self.ctx.symbols.contains_key(name)   // a `using` is required for an import
            && self.ctx.natives.module_properties.contains(&format!("{name}.{member}")) // And it's a property
        {
            self.emit(bc::Op::CallNative(bc::TaskCallOperand {
                target: Some(name.clone()),
                name: member.to_string(),
                argument_count: 0,
                is_member_access: false,
            }));
            return Ok(());
        }

        self.compile_expression(target)?;

        if access_type != bc::MemberAccessType::None {
            self.emit(bc::Op::MemberAccess(bc::MemberAccessOperand {
                member_name: member.to_string(),
                access_type,
            }));
            return Ok(());
        }

        // An instance property goes through its native getter, with the target as `self`
        self.emit(bc::Op::CallNative(bc::TaskCallOperand {
            target: None,
            name: member.to_string(),
            argument_count: 1,
            is_member_access: true,
        }));
        Ok(())
    }

    fn compile_lambda(&mut self, lambda: &Lambda, span: Span) -> Result<(), MaxDiagnostic> {
        // Claim the ID first, so an outer lambda is numbered before the lambdas inside it
        let id = self.ctx.alloc_lambda_id();

        // The body is compiled by a separate compiler, so it gets its own instruction list
        let mut child = self.new_child();

        for scope in &self.variable_scopes {
            for (name, info) in scope {
                child.variable_scopes[0].insert(name.clone(), *info);
            }
        }
        for parameter in &lambda.parameters {
            let name = parameter.name.node.clone();
            child.variable_scopes[0].insert(name, VarInfo::default());
        }

        match &lambda.body {
            LambdaBody::Expression(expression) => {
                child.compile_expression(expression).map(|_| child.emit(bc::Op::ReturnValue))?;
            }
            LambdaBody::Block(body) => {
                if !contains_return(body) {
                    return Err(error(span, "missing-lambda-return", "A block-bodied lambda must contain a return statement."));
                }
                child.compile_statements(body)?;
                child.emit(bc::Op::Return);
            }
        }

        let captured = self.collect_captures(&child.ops, &lambda.parameters);
        let as_other_params: Vec<max_bytecode::Parameter> = lambda.parameters.iter().map(
            |param| max_bytecode::Parameter {
                name: param.name.node.clone(),
                // ALWAYS Required!
                parameter_kind: max_bytecode::ParamKind::Required(
                    // Convert the TypeRef
                    self.convert_optional_type(&param.type_ref).unwrap_or(None)
                )
            }
        ).collect();

        self.emit(bc::Op::Lambda(Arc::new(bc::LambdaOperand {
            id,
            arity: lambda.parameters.len() as u64,
            parameters: as_other_params,
            captured,
            chunk: Arc::new(bc::Chunk { instructions: Arc::new(child.ops) }),
        })));
        Ok(())
    }

    /// The variables a lambda body uses but does not define itself
    fn collect_captures(&self, ops: &[bc::Op], parameters: &[Parameter]) -> Vec<String> {
        // Parameters and anything the lambda stores are its own
        let mut locals: HashSet<&str> = parameters.iter().map(|par| String::as_str(&par.name.node)).collect();
        for op in ops {
            if let bc::Op::Store(store) = op {
                locals.insert(store.name.as_str());
            }
        }

        // Every instruction that mentions a variable by name
        let mut references: Vec<&str> = Vec::new();
        for op in ops {
            match op {
                bc::Op::Load(name) => references.push(name),
                bc::Op::Update(update) => references.push(&update.name),
                bc::Op::UpdateIndex(update) => {
                    if let Some(name) = &update.target_name {
                        references.push(name);
                    }
                }
                bc::Op::Lambda(nested) => references.extend(nested.captured.iter().map(String::as_str)),
                // Calling a lambda that is stored in a variable also uses that variable
                bc::Op::Call(call) if call.target.is_none() && self.is_variable_declared(&call.name) => {
                    references.push(&call.name)
                }
                _ => {}
            }
        }

        let mut captured: Vec<String> = Vec::new();
        for name in references {
            // Enums and modules are not variables, so there is nothing to capture
            if locals.contains(name) || captured.iter().any(|c| c == name) || self.is_compile_time_name(name) {
                continue;
            }
            captured.push(name.to_string());
        }
        captured
    }

    /// Checks for if a 'this' expression is valid. This is connected to the
    /// Compiler's `model_context`. No model context or if the context is not
    /// inside an instance task (`!in_instance_task`) means 'this' is invalid.
    fn check_this(&self, span: Span) -> Result<(), MaxDiagnostic> {
        match &self.model_context {
            None => Err(error(span, "this-outside-object", "'this' can only be used inside an object's instance tasks")),
            
            Some(c) if !c.in_instance_task => Err(error(span, "this-in-static-context",
                "'this' cannot be used inside static tasks or object initialisers")),
            
            Some(_) => Ok(()),
        }
    }

    /// Function that runs a series of checks that return an error diagnostic if
    /// any fail. This function errors in the following situations:
    /// 
    /// - `this` outside objects
    /// - `this` accessing unknown member
    /// - Using `this` to access a static member
    /// - The member accessed does match the `MemberKind` passed in
    /// 
    /// `this.member` is fully checkable: the object is the one being compiled
    fn check_this_member(&self, member: &str, kind: MemberKind, span: Span) -> Result<(), MaxDiagnostic> {
        // Cannot use 'this' outside an object
        let Some(object) = self.model_context.as_ref().and_then(|c| self.local_objects.get(&c.name)) else {
            return Err(error(span, "this-outside-object", "'this' can only be used inside an object"));
        };
        // Simply does not exist
        let Some(found) = object.members.iter().find(|m| m.name == member) else {
            return Err(error(span, "unknown-object-member", format!("'{member}' is not a member of '{}'", object.name)));
        };
        // Static but tried to be accessed with 'this'
        if found.is_static {
            return Err(error(span, "static-member-via-this",
                format!("'{member}' is static; access it as '{}.{member}'", object.name)));
        }
        // Not matching the wanted member kind
        if found.kind != kind {
            return Err(error(span, "wrong-member-kind", format!("'{member}' is not a {}",
                if kind == MemberKind::Field { "field" } else { "task" })));
        }
        Ok(())
    }
}

// ---------------------------- //
// ----- FREE HELPER FUNCS ---- //
// ---------------------------- //

pub fn convert_modifiers(modifiers: &ModifierSet) -> bc::Modifiers {
    bc::Modifiers {
        open: modifiers.lifecycle == Some(Lifecycle::Open),
        monic: modifiers.lifecycle == Some(Lifecycle::Monic),
        fixed: modifiers.mutability == Some(Mutability::Fixed),
    }
}

/// Converts the given `ast::Visibility` into `max_bytecode::Visibility`
fn convert_visibility_to_bytecode(visibility: &Option<Visibility>) -> max_bytecode::Visibility {
    match visibility {
        Some(visible) => match visible {
            Visibility::Public => max_bytecode::Visibility::Public,
            Visibility::Private => max_bytecode::Visibility::Private,
            Visibility::Protected => unreachable!("This should not be reached")
        }

        // Our default!
        None => max_bytecode::Visibility::Public
    }
}

fn compare_op(op: ComparisonOp) -> bc::CompareOp {
    match op {
        ComparisonOp::Equal => bc::CompareOp::Equal,
        ComparisonOp::NotEqual => bc::CompareOp::NotEqual,
        ComparisonOp::Lesser => bc::CompareOp::Lesser,
        ComparisonOp::LesserEqual => bc::CompareOp::LesserEqual,
        ComparisonOp::Greater => bc::CompareOp::Greater,
        ComparisonOp::GreaterEqual => bc::CompareOp::GreaterEqual,
    }
}

fn update_operation(op: AssignOp) -> bc::UpdateOperation {
    match op {
        AssignOp::Assign => bc::UpdateOperation::Assign,
        AssignOp::Add => bc::UpdateOperation::Add,
        AssignOp::Subtract => bc::UpdateOperation::Subtract,
        AssignOp::Multiply => bc::UpdateOperation::Multiply,
        AssignOp::Divide => bc::UpdateOperation::Divide,
        AssignOp::Modulus => bc::UpdateOperation::Modulus,
    }
}

/**
    Compiles a `ExpressionKind` into it's respective `ConstValue`. Not all ExpressionKinds resolve into a ConstValue.
    Note: A `resort` value is stored in the bytecode itself, so it must be a literal

    ## Examples

    ```
    # use bc::ConstValue;
    # use crate::ast::ExpressionKind;

    let expression = ExpressionKind::Integer(5);
    let constant = to_const(expression);

    assert_eq!(constant, ConstValue::Integer(5));

    # let _: Option<&str>;
    ```

    Unary Expressions can also be targetted, specifically `Negate`  

    ```
    # use bc::ConstValue;
    # use crate::ast::{ExpressionKind, UnaryOp};
    let expression = ExpressionKind::Unary {
        op: UnaryOp::Negate, 
        operand: Box::new(
            Expression::new(
                ExpressionKind::Integer(10),
                Span { line: 1, column: 1 }
            )
        )
    };
    let constant = to_const(expression);

    assert_eq!(constant, ConstValue::Integer(-10));
    ```
*/ 
fn to_const(expression: &Expression) -> Result<bc::ConstValue, MaxDiagnostic> {
    Ok(match &expression.kind {
        ExpressionKind::Integer(value) => bc::ConstValue::Integer(*value),
        ExpressionKind::Float(value) => bc::ConstValue::Float(*value),
        ExpressionKind::Str(value) => bc::ConstValue::Str(value.clone()),
        ExpressionKind::Boolean(value) => bc::ConstValue::Boolean(*value),
        ExpressionKind::Possible => bc::ConstValue::Possible,
        ExpressionKind::Nil => bc::ConstValue::Nil,

        // Negative numbers parse as a negation of a literal
        ExpressionKind::Unary { op: UnaryOp::Negate, operand } => match &operand.kind {
            ExpressionKind::Integer(value) => bc::ConstValue::Integer(-*value),
            ExpressionKind::Float(value) => bc::ConstValue::Float(-*value),
            _ => return Err(error(expression.span, "not-literal-resort", "A resort value must be a literal")),
        },

        ExpressionKind::List(items) => {
            bc::ConstValue::List(items.iter().map(to_const).collect::<Result<Vec<_>, _>>()?)
        }
        ExpressionKind::Dictionary(entries) => bc::ConstValue::Dictionary(
            entries
                .iter()
                .map(|(key, value)| Ok((to_const(key)?, to_const(value)?)))
                .collect::<Result<Vec<_>, MaxDiagnostic>>()?,
        ),

        _ => return Err(error(expression.span, "not-literal-resort", "A resort value must be a literal")),
    })
}

/// Does this block contain a `return`, including inside its if, while, for and foreach bodies?
fn contains_return(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        StatementKind::Return(_) => true,
        StatementKind::If(branch) => {
            contains_return(&branch.first.body)
                || branch.elifs.iter().any(|b| contains_return(&b.body))
                || contains_return(&branch.else_block)
                || contains_return(&branch.maybe_block)
        }
        StatementKind::While(w) => contains_return(&w.body),
        StatementKind::For(f) => contains_return(&f.body),
        StatementKind::Foreach(f) => contains_return(&f.body),
        _ => false,
    })
}

/**
Builds the `ParamSig` list for a task definition, in declaration order.

Used in two places: pass 1 of `compile()` (so calls can be bound even to tasks defined
LATER in the same file) and `SymbolTable::from_statements` (so other files can bind
calls to tasks they import).
Unlike `lookup_signature` in [`compiler`][`Compiler`], this directly intakes a task definition,
meaning there will be a known vector of `ParamSig`, even if it is empty
*/
fn signature_of_task(task: &TaskDefinition) -> Vec<ParamSignature> {
    task.parameters.iter().map(|p| ParamSignature { name: p.name.node.clone(), kind: p.kind.clone() }).collect()
}

/// Infers return provenance for each function, iterating until forwarding calls stabilize.
///
/// The result is conservative across branches: if any return path can yield monic data,
/// that possibility is recorded in the task summary.
fn monic_returning_tasks(
    statements: &[Statement],
    dependencies: &[String],
    symbols: &HashMap<String, SymbolTable>,
) -> HashMap<String, MonicReturnSummary> {
    // Collect all them tasks baby!
    let (mut tasks, mut monics): (Vec<&TaskDefinition>, Vec<&Declaration>) = (Vec::new(), Vec::new());
    for statement in statements {
        match &statement.kind {
            // Events do not return anything!
            StatementKind::TaskDefinition(task) if task.kind == TaskKind::Function || task.kind == TaskKind::Coroutine => tasks.push(task),
            // Push only monics
            StatementKind::Declaration(declaration) if declaration.modifiers.lifecycle == Some(Lifecycle::Monic) => monics.push(declaration),
            _ => {},
        };
    }

    // Collect signatures
    let local_signatures: HashMap<String, Vec<ParamSignature>> = tasks.iter()
        .map(|task| (task.name.clone(), signature_of_task(task)))
        .collect();
    let mut summaries = HashMap::new();

    loop {
        let previous = summaries.clone();
        for task in &tasks {
            let mut origins = HashMap::new();
            for (index, parameter) in task.parameters.iter().enumerate() {
                // Create a monic origin with only the param name and it's usize position
                origins.insert(parameter.name.node.clone(), MonicOrigin {
                    always_monic: false,
                    forwarded_parameters: HashSet::from([index]),
                });
            }

            for monic in monics.iter() {
                origins.insert(monic.name.clone(), MonicOrigin {
                    always_monic: true,
                    forwarded_parameters: Default::default()
                });
            }

            // Attempt to identify a return statement with monic value
            let returned = monic_return_origin_in_statements(
                &task.body, &mut origins, &local_signatures, &summaries, dependencies, symbols,
            );

            let summary = MonicReturnSummary {
                always_monic: returned.always_monic,
                forwarded_parameters: returned.forwarded_parameters,
            };
            summaries.entry(task.name.clone()).or_insert_with(MonicReturnSummary::default).merge(summary);
        }
        if summaries == previous {
            break;
        }
    }

    summaries
}

/// Finds a known user-task signature and its monic return summary, local or imported.
/// `local_signatures` and `local_summaries` are used to find the signature and
/// `MonicReturnSummary` from a name. The Compiler's `get_from_member_access` can be
/// seen in this function as well as others in order to break member access to the
/// core variable to access it's monomorphism, if possible.
/// 
/// Due to this function requiring these params, consequential functions that use this
/// **must also hold the locals or get access to them**.
fn monic_call_summary(
    callee: &Expression,
    local_signatures: &HashMap<String, Vec<ParamSignature>>,
    local_summaries: &HashMap<String, MonicReturnSummary>,
    dependencies: &[String],
    symbols: &HashMap<String, SymbolTable>,
) -> Option<(Vec<ParamSignature>, MonicReturnSummary)> {
    match &callee.kind {
        // Why how kind to be so simple!
        ExpressionKind::Variable(name) => {
            if let (Some(signature), Some(summary)) = (local_signatures.get(name), local_summaries.get(name)) {
                return Some((signature.clone(), summary.clone()));
            }
            for dependency in dependencies {
                let Some(table) = symbols.get(dependency) else { continue };
                if let (Some(SymbolInfo::Task(signature)), Some(summary)) =
                    (table.contents.get(name), table.monic_returns.get(name))
                {
                    return Some((signature.clone(), summary.clone()));
                }
            }
            None
        }
        // Oh great, more member access...
        ExpressionKind::MemberAccess { target, member } => {
            let Some(expr) = &Compiler::get_from_member_access(target)
                else { return None;};

            let ExpressionKind::Variable(module) = &expr.kind else { return None; };

            if !dependencies.iter().any(|dependency| dependency == module) {
                return None;
            }

            let table = symbols.get(module)?;
            let SymbolInfo::Task(signature) = table.contents.get(member)? else { return None };
            Some((signature.clone(), table.monic_returns.get(member)?.clone()))
        }
        _ => None,
    }
}

/// Maps call arguments to parameter positions for return-provenance substitution. The returned
/// `Vec<Vec<&'a Expression>>` is built like so to represent the different param types. Optional
/// params may have an empty Vec, whilst Variadic params can have as many arguments passed into
/// their final Vec as they want.
///
/// This mirrors named, positional, and variadic argument binding without emitting bytecode.
fn bind_monic_arguments<'a>(arguments: &'a [Argument], parameters: &[ParamSignature]) -> Vec<Vec<&'a Expression>> {
    // Quick calcs for the variadic and fixed counts
    let variadic = parameters.last().is_some_and(|parameter| parameter.kind == ParameterKind::Variadic);
    let fixed_count = parameters.len() - usize::from(variadic);
    // A vector of vectors of expressions. The parent vector has the length of the parameters length
    let mut bound = vec![Vec::new(); parameters.len()];

    // First we simply push our named arguments as the normal would...
    for argument in arguments {
        let Some(name) = &argument.name else { continue };
        if let Some(index) = parameters[..fixed_count].iter().position(|parameter| parameter.name == name.node) {
            bound[index].push(&argument.value);
        }
    }

    // The rest are of course put into the optionals or the variadics that may exist
    let mut free = (0..fixed_count).filter(|index| /* Only the unassigned! */ bound[*index].is_empty())
        .collect::<Vec<_>>().into_iter();

    for argument in arguments.iter().filter(|argument| argument.name.is_none()) {
        // Now push!
        if let Some(index) = free.next() {
            bound[index].push(&argument.value);
        } else if variadic {
            bound[parameters.len() - 1].push(&argument.value);
        }
    }

    bound
}

/// Computes monic provenance for an expression while analyzing a task body.
/// The `local_signatures` and `local_summaries`
///
/// Operations that combine or expose their operands retain provenance. A property read
/// or a call without a direct-return summary represents a derived value and clears it.
fn monic_origin_in_expression(
    expression: &Expression,
    variables: &HashMap<String, MonicOrigin>,
    local_signatures: &HashMap<String, Vec<ParamSignature>>,
    local_summaries: &HashMap<String, MonicReturnSummary>,
    dependencies: &[String],
    symbols: &HashMap<String, SymbolTable>,
) -> MonicOrigin {
    let mut origin = MonicOrigin::default();
    match &expression.kind {
        /*
            Like many processes, this simply just recursively finds a monic origin within an expression
        */
        ExpressionKind::Variable(name) => {
            if let Some(found) = variables.get(name) {
                origin.merge(found.clone());
            }
        }
        ExpressionKind::Unary { operand, .. } => origin.merge(monic_origin_in_expression(
            operand, variables, local_signatures, local_summaries, dependencies, symbols,
        )),
        ExpressionKind::Binary { left, right, .. }
        | ExpressionKind::Comparison { left, right, .. }
        | ExpressionKind::Logical { left, right, .. } => {
            origin.merge(monic_origin_in_expression(left, variables, local_signatures, local_summaries, dependencies, symbols));
            origin.merge(monic_origin_in_expression(right, variables, local_signatures, local_summaries, dependencies, symbols));
        }
        ExpressionKind::List(items) => {
            for item in items {
                origin.merge(monic_origin_in_expression(item, variables, local_signatures, local_summaries, dependencies, symbols));
            }
        }
        ExpressionKind::Dictionary(entries) => {
            for (key, value) in entries {
                origin.merge(monic_origin_in_expression(key, variables, local_signatures, local_summaries, dependencies, symbols));
                origin.merge(monic_origin_in_expression(value, variables, local_signatures, local_summaries, dependencies, symbols));
            }
        }
        ExpressionKind::Index { target, .. } => {
            origin.merge(monic_origin_in_expression(target, variables, local_signatures, local_summaries, dependencies, symbols));
        }
        ExpressionKind::Callable { callee, arguments } => {
            let shadowed = match &callee.kind {
                // Simply check check check!
                ExpressionKind::Variable(name) => variables.contains_key(name),
                ExpressionKind::MemberAccess { target, .. } => matches!(
                    // Break down to find the final member
                    &Compiler::get_from_member_access(target).unwrap_or(Expression::not_real(ExpressionKind::Nil)).kind,
                    ExpressionKind::Variable(var_name) if variables.contains_key(var_name)
                ),
                _ => false,
            };

            //
            if !shadowed && let Some((parameters, summary)) = monic_call_summary(
                callee, local_signatures, local_summaries, dependencies, symbols,
            ) {
                origin.always_monic = summary.always_monic;
                let bound = bind_monic_arguments(arguments, &parameters);
                for index in summary.forwarded_parameters {
                    if let Some(values) = bound.get(index) {
                        for value in values {
                            // Get all the fowarded params in!
                            origin.merge(monic_origin_in_expression(
                                value, variables, local_signatures, local_summaries, dependencies, symbols,
                            ));
                        }
                    }
                }
            }
        }

        _ => {}
    }
    origin
}

/// Collects possible monic origins from returns in a task's statements and nested blocks.
///
/// Each branch is analyzed with a copy of the current bindings so declarations in one
/// branch do not leak into another.
fn monic_return_origin_in_statements(
    statements: &[Statement],
    variables: &mut HashMap<String, MonicOrigin>,
    local_signatures: &HashMap<String, Vec<ParamSignature>>,
    local_summaries: &HashMap<String, MonicReturnSummary>,
    dependencies: &[String],
    symbols: &HashMap<String, SymbolTable>,
) -> MonicOrigin {
    let mut returned = MonicOrigin::default();
    for statement in statements {
        match &statement.kind {
            /*
                Note: This will be a very long, very big list...
            */
            StatementKind::Declaration(declaration) => {
                if declaration.modifiers.lifecycle == Some(Lifecycle::Monic) {
                    variables.insert(declaration.name.clone(), MonicOrigin { always_monic: true, forwarded_parameters: HashSet::new() });
                } else {
                    variables.insert(declaration.name.clone(), MonicOrigin::default());
                }
            }
            StatementKind::Return(Some(expression)) => returned.merge(monic_origin_in_expression(
                expression, variables, local_signatures, local_summaries, dependencies, symbols,
            )),
            StatementKind::If(branch) => {
                let mut first = variables.clone();
                returned.merge(monic_return_origin_in_statements(&branch.first.body, &mut first, local_signatures, local_summaries, dependencies, symbols));
                for conditional in &branch.elifs {
                    let mut nested = variables.clone();
                    returned.merge(monic_return_origin_in_statements(&conditional.body, &mut nested, local_signatures, local_summaries, dependencies, symbols));
                }
                let mut maybe = variables.clone();
                returned.merge(monic_return_origin_in_statements(&branch.maybe_block, &mut maybe, local_signatures, local_summaries, dependencies, symbols));
                let mut otherwise = variables.clone();
                returned.merge(monic_return_origin_in_statements(&branch.else_block, &mut otherwise, local_signatures, local_summaries, dependencies, symbols));
            }
            StatementKind::While(loop_statement) => {
                let mut nested = variables.clone();
                returned.merge(monic_return_origin_in_statements(&loop_statement.body, &mut nested, local_signatures, local_summaries, dependencies, symbols));
            }
            StatementKind::For(loop_statement) => {
                let mut nested = variables.clone();
                // Who knows! Maybe we'll allow monic variables in for loops...
                if loop_statement.declaration.modifiers.lifecycle == Some(Lifecycle::Monic) {
                    nested.insert(loop_statement.declaration.name.clone(), MonicOrigin { always_monic: true, forwarded_parameters: HashSet::new() });
                }
                returned.merge(monic_return_origin_in_statements(&loop_statement.body, &mut nested, local_signatures, local_summaries, dependencies, symbols));
            }
            StatementKind::Foreach(loop_statement) => {
                let mut nested = variables.clone();
                // Variable(s) don't specify monomorphism anyway
                for name in &loop_statement.variables { nested.insert(name.clone(), MonicOrigin::default()); }
                returned.merge(monic_return_origin_in_statements(&loop_statement.body, &mut nested, local_signatures, local_summaries, dependencies, symbols));
            }
            StatementKind::Match(match_statement) => {
                for case in &match_statement.cases {
                    let mut nested = variables.clone();
                    returned.merge(monic_return_origin_in_statements(&case.body, &mut nested, local_signatures, local_summaries, dependencies, symbols));
                }
                if let Some(default) = &match_statement.default {
                    let mut nested = variables.clone();
                    returned.merge(monic_return_origin_in_statements(default, &mut nested, local_signatures, local_summaries, dependencies, symbols));
                }
            }

            _ => {}
        }
    }
    returned
}