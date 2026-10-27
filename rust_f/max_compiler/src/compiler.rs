use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use max_bytecode as bc;

use crate::{ast::*, error::MaxError};

fn error(span: Span, message: impl Into<String>) -> MaxError {
    MaxError::Syntax { message: message.into(), line: span.line, column: span.column }
}

// ---------------------------- //
// ----- SHARED CONTEXT ------- //
// ---------------------------- //

// When we say shared, think of the keyword 'using'

/// The names the compiler needs from the native registry
#[derive(Debug, Default, Clone)]
pub struct NativeNames {
    pub static_modules: HashSet<String>,
    pub global_tasks: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum SymbolInfo {
    Task,
    Coroutine,
    Enum(Vec<EnumMember>),
}

/// What a module exposes to the files that `using` it
#[derive(Debug, Clone)]
pub struct SymbolTable {
    pub module_name: String,
    pub contents: HashMap<String, SymbolInfo>,
}

impl SymbolTable {
    pub fn from_statements(module_name: &str, statements: &[Statement]) -> Self {
        let mut contents = HashMap::new();

        for statement in statements {
            match &statement.kind {
                StatementKind::TaskDefinition(task) => {
                    let info = match task.kind {
                        TaskKind::Coroutine => SymbolInfo::Coroutine,
                        TaskKind::Function => SymbolInfo::Task,
                    };
                    contents.insert(task.name.clone(), info);
                }
                StatementKind::Enum(definition) => {
                    contents.insert(definition.identifier.clone(), SymbolInfo::Enum(definition.members.clone()));
                }
                _ => {}
            }
        }

        SymbolTable { module_name: module_name.to_string(), contents }
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
}

enum NodeKind {
    Definition,
    Member,
}

/// What an expression such as `ItemType` or `ItemType.Tool` statically refers to
struct EnumNode<'s> {
    kind: NodeKind,
    path: String,
    children: &'s [EnumMember],
}

pub struct Compiler<'a> {
    /// Our CompileContext for the Compiler
    ctx: &'a CompileContext,
    dependencies: &'a [String],

    ops: Vec<bc::Op>,
    tasks: HashMap<String, bc::TaskInfo>,
    coroutines: HashMap<String, bc::CoroutineInfo>,
    enums: HashMap<String, bc::EnumStructure>,

    /// Every enum in this file (collected before compiling), as the parser produced them
    local_enums: HashMap<String, Vec<EnumMember>>,

    /// 0 at the top level, above 0 inside a task or lambda
    task_depth: u32,
    variable_scopes: Vec<HashMap<String, VarInfo>>,
}

impl<'a> Compiler<'a> {
    pub fn new(ctx: &'a CompileContext, dependencies: &'a [String]) -> Self {
        Compiler {
            ctx,
            dependencies,
            ops: Vec::new(),
            tasks: HashMap::new(),
            coroutines: HashMap::new(),
            enums: HashMap::new(),
            local_enums: HashMap::new(),
            task_depth: 0,
            variable_scopes: vec![HashMap::new()],
        }
    }

    /// A compiler for a lambda body. It sees what the parent has defined so far,
    /// but its instructions go into its own list.
    fn new_child(&self) -> Compiler<'a> {
        Compiler {
            ctx: self.ctx,
            dependencies: self.dependencies,
            ops: Vec::new(),
            tasks: self.tasks.clone(),
            coroutines: self.coroutines.clone(),
            enums: self.enums.clone(),
            local_enums: self.local_enums.clone(),
            task_depth: 1,
            variable_scopes: vec![HashMap::new()],
        }
    }

    pub fn compile(mut self, statements: &[Statement]) -> Result<bc::Bytecode, MaxError> {
        // Pass 1: learn every top-level enum, so types and member chains can refer to enums defined later
        for statement in statements {
            if let StatementKind::Enum(definition) = &statement.kind {
                self.local_enums.insert(definition.identifier.clone(), definition.members.clone());
            }
        }

        // Pass 2: emit all enum definitions first, so they exist before any code runs
        for statement in statements {
            if let StatementKind::Enum(definition) = &statement.kind {
                self.compile_enum_definition(definition)?;
            }
        }

        // Pass 3: everything else, in order
        for statement in statements {
            if !matches!(statement.kind, StatementKind::Enum(_)) {
                self.compile_statement(statement)?;
            }
        }

        Ok(bc::Bytecode {
            instructions: Arc::new(self.ops),
            tasks: Arc::new(self.tasks),
            coroutines: Arc::new(self.coroutines),
            enums: Arc::new(self.enums),
        })
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

    fn compile_statements(&mut self, statements: &[Statement]) -> Result<(), MaxError> {
        for statement in statements {
            self.compile_statement(statement)?;
        }
        Ok(())
    }

    /// `EnterScope`, the body, `ExitScope`, with the compiler's own scopes kept in step
    fn compile_scoped_block(&mut self, body: &[Statement]) -> Result<(), MaxError> {
        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();
        self.compile_statements(body)?;
        self.emit(bc::Op::ExitScope);
        self.exit_compiler_scope();
        Ok(())
    }

    // ---------------------------- //
    // ------- NAME CHECKS -------- //
    // ---------------------------- //

    fn is_variable_declared(&self, name: &str) -> bool {
        self.variable_scopes.iter().any(|scope| scope.contains_key(name))
    }

    /// A task, coroutine, enum, native global or non-open variable already uses this name
    fn has_duplicate_naming(&self, name: &str) -> bool {
        if self.tasks.contains_key(name)
            || self.coroutines.contains_key(name)
            || self.enums.contains_key(name)
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
                || self.ctx.symbols.contains_key(name))
    }

    // ---------------------------- //
    // ---------- TYPES ----------- //
    // ---------------------------- //

    fn convert_type(&self, type_ref: &TypeRef, span: Span) -> Result<bc::MaxType, MaxError> {
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
                if self.find_enum_members(name).is_none() {
                    return Err(error(span, format!("Unknown type '{name}'")));
                }
                bc::MaxType::EnumType(name.clone())
            }
        })
    }

    fn convert_optional_type(&self, type_ref: &Option<Spanned<TypeRef>>) -> Result<Option<bc::MaxType>, MaxError> {
        match type_ref {
            Some(spanned) => Ok(Some(self.convert_type(&spanned.node, spanned.span)?)),
            None => Ok(None),
        }
    }

    fn convert_parameters(&self, parameters: &[Parameter]) -> Result<Vec<bc::Parameter>, MaxError> {
        parameters
            .iter()
            .map(|p| Ok(bc::Parameter { max_type: self.convert_optional_type(&p.type_ref)?, name: p.name.clone() }))
            .collect()
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
    fn sub_members_of<'s>(&'s self, member: &'s EnumMember, span: Span) -> Result<&'s [EnumMember], MaxError> {
        match &member.sub_enum_reference {
            Some(reference) => self.find_enum_members(reference).ok_or_else(|| {
                error(span, format!("Sub enum '{reference}' referenced by '{}' was not found", member.name))
            }),
            None => Ok(member.sub_members.as_slice()),
        }
    }

    /// Works out what an expression refers to inside the enum hierarchy, one step at a time
    fn resolve_enum_node<'s>(&'s self, expression: &'s Expression) -> Result<Option<EnumNode<'s>>, MaxError> {
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
                    .ok_or_else(|| error(expression.span, format!("'{member}' is not a member of '{}'", parent.path)))?;

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
    ) -> Result<bc::MemberAccessType, MaxError> {
        // Take in steps, just don't trip!
        // 1. An enum definition or an enum member
        if let Some(node) = self.resolve_enum_node(target)? {
            if !node.children.iter().any(|m| m.name == member) {
                return Err(error(span, format!("'{member}' is not a member of '{}'", node.path)));
            }
            return Ok(match node.kind {
                NodeKind::Definition => bc::MemberAccessType::Enum,
                NodeKind::Member => bc::MemberAccessType::EnumMember,
            });
        }

        // 2. Module access (map.Direction)
        if let ExpressionKind::Variable(name) = &target.kind
            && !self.is_variable_declared(name)
            && self.ctx.symbols.contains_key(name)
        {
            return Ok(bc::MemberAccessType::Module);
        }

        // A memory of our C# Compiler😭: a nested access whose member is itself a module name
        if let ExpressionKind::MemberAccess { member: nested, .. } = &target.kind
            && self.ctx.symbols.contains_key(nested)
        {
            return Ok(bc::MemberAccessType::Module);
        }

        // 3. Anything else is an instance property
        Ok(bc::MemberAccessType::None)
    }

    fn compile_enum_definition(&mut self, definition: &EnumDefinition) -> Result<(), MaxError> {
        if self.has_duplicate_naming(&definition.identifier) {
            return Err(error(
                definition.name_span,
                format!("Duplicate naming found when attempting to create enum '{}'", definition.identifier),
            ));
        }

        // Nested enum definitions are not hoisted, so make sure they are known
        self.local_enums
            .entry(definition.identifier.clone())
            .or_insert_with(|| definition.members.clone());

        let members = self.build_members(&definition.identifier, &[], &definition.members)?;
        let structure = bc::EnumStructure { identifier: definition.identifier.clone(), enum_members: members };

        self.enums.insert(definition.identifier.clone(), structure.clone());
        self.emit(bc::Op::DefineEnum(structure));
        Ok(())
    }

    /// Fills in each member's enum name and path, and checks for duplicates and missing sub enums
    fn build_members(
        &self,
        enum_name: &str,
        parent_path: &[String],
        members: &[EnumMember],
    ) -> Result<Vec<bc::EnumMember>, MaxError> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut built = Vec::with_capacity(members.len());

        for member in members {
            if !seen.insert(member.name.as_str()) {
                return Err(error(
                    member.span,
                    format!("Duplicate enum member found '{}' in '{enum_name}'", member.name),
                ));
            }

            if let Some(reference) = &member.sub_enum_reference
                && self.find_enum_members(reference).is_none()
            {
                return Err(error(member.span, format!("Sub enum '{reference}' was not found")));
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

    fn compile_statement(&mut self, statement: &Statement) -> Result<(), MaxError> {
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

            StatementKind::Expression(expression) => {
                self.compile_expression(expression)?;
                self.emit(bc::Op::Pop);
            }

            StatementKind::Return(value) => self.compile_return(value.as_ref(), statement.span)?,
            StatementKind::Yield(value) => self.compile_yield(value.as_ref())?,
            StatementKind::TaskDefinition(task) => self.compile_task_definition(task)?,
            StatementKind::Enum(definition) => self.compile_enum_definition(definition)?,
        }
        Ok(())
    }

    fn compile_declaration(&mut self, declaration: &Declaration) -> Result<(), MaxError> {
        if self.task_depth == 0 {
            if self.has_duplicate_naming(&declaration.name) {
                return Err(error(
                    declaration.name_span,
                    format!("A task or variable of the same name has already been defined: {}", declaration.name),
                ));
            }

            let open = declaration.modifiers.lifecycle == Some(Lifecycle::Open);
            self.variable_scopes
                .last_mut()
                .expect("the compiler always has a scope")
                .insert(declaration.name.clone(), VarInfo { open });
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

    fn compile_assignment(&mut self, assignment: &Assignment) -> Result<(), MaxError> {
        let operation = update_operation(assignment.op);

        match &assignment.target.kind {
            ExpressionKind::Variable(name) => {
                self.compile_expression(&assignment.value)?;
                self.emit(bc::Op::Update(bc::UpdateOperand { name: name.clone(), operation }));
            }

            ExpressionKind::Index { target, index } => {
                self.compile_expression(target)?;
                self.compile_expression(index)?;
                self.compile_expression(&assignment.value)?;

                let target_name = match &target.kind {
                    ExpressionKind::Variable(name) => Some(name.clone()),
                    _ => None,
                };
                self.emit(bc::Op::UpdateIndex(bc::UpdateIndexOperand { operation, target_name }));
            }

            _ => return Err(error(assignment.target.span, "This expression cannot be assigned to")),
        }
        Ok(())
    }

    fn compile_if(&mut self, statement: &IfStatement) -> Result<(), MaxError> {
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

    fn compile_while(&mut self, statement: &WhileStatement) -> Result<(), MaxError> {
        // The condition is re-checked at the start of every iteration
        let condition_start = self.ops.len();

        self.compile_expression(&statement.condition)?;
        let exit_jump = self.emit_jump(bc::Op::JumpIfFalse);

        self.compile_scoped_block(&statement.body)?;
        self.emit(bc::Op::Jump(condition_start));

        let exit = self.ops.len();
        self.patch_jump(exit_jump, exit);
        Ok(())
    }

    fn compile_for(&mut self, statement: &ForStatement) -> Result<(), MaxError> {
        // This will be a very linear set of instructions...
        let name = &statement.declaration.name;

        // The loop variable lives in a scope of its own
        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

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

        self.compile_scoped_block(&statement.body)?;

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
        Ok(())
    }

    /// Pushes `slot <op> 0`, which decides how a `for` loop counts
    fn compile_temporary_comparison(&mut self, slot: u64, op: bc::CompareOp) {
        self.emit(bc::Op::TemporaryLoad(slot));
        self.emit_constant(bc::ConstValue::Integer(0));
        self.emit(bc::Op::Compare(op));
    }

    /// Pushes `variable <op> limit`
    fn compile_for_bound_check(&mut self, name: &str, limit: &Expression, op: bc::CompareOp) -> Result<(), MaxError> {
        self.emit(bc::Op::Load(name.to_string()));
        self.compile_expression(limit)?;
        self.emit(bc::Op::Compare(op));
        Ok(())
    }

    fn compile_foreach(&mut self, statement: &ForeachStatement) -> Result<(), MaxError> {
        // Evaluate the collection once and keep it in a temporary slot
        let collection_slot = self.ctx.alloc_slot();
        self.compile_expression(&statement.iterable)?;
        self.emit(bc::Op::TemporaryStore(collection_slot));

        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

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

        // Each iteration gets its own scope, so `declare` inside the body works every time
        self.compile_scoped_block(&statement.body)?;
        self.emit(bc::Op::Jump(loop_start));

        let exit = self.ops.len();
        self.emit(bc::Op::ExitScope);
        self.exit_compiler_scope();
        self.patch_jump(exit_jump, exit);
        Ok(())
    }

    fn compile_return(&mut self, value: Option<&Expression>, span: Span) -> Result<(), MaxError> {
        if self.task_depth == 0 {
            return Err(error(span, "Return statements can only be used inside a task."));
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

    fn compile_yield(&mut self, value: Option<&Expression>) -> Result<(), MaxError> {
        match value {
            Some(expression) => self.compile_expression(expression)?,
            None => self.emit_constant(bc::ConstValue::Nil),
        }
        self.emit(bc::Op::Yield);
        Ok(())
    }

    // ---------------------------- //
    // ---------- TASKS ----------- //
    // ---------------------------- //

    fn compile_task_definition(&mut self, task: &TaskDefinition) -> Result<(), MaxError> {
        if self.has_duplicate_naming(&task.name) {
            return Err(error(
                task.name_span,
                format!("A task or variable of the same name has already been defined: {}", task.name),
            ));
        }

        let return_type = self.convert_optional_type(&task.return_type)?;

        // Jump over the body when execution reaches the definition itself
        let skip_body = self.emit_jump(bc::Op::Jump);

        self.emit(bc::Op::EnterScope);
        self.enter_compiler_scope();

        let entry_point = self.ops.len() as u64;

        match task.kind {
            TaskKind::Function => {
                let parameters = self.convert_parameters(&task.parameters)?;

                // Registered before the body, so the task can call itself
                self.tasks.insert(
                    task.name.clone(),
                    bc::TaskInfo { entry_point, parameters, return_type },
                );

                for parameter in &task.parameters {
                    self.variable_scopes
                        .last_mut()
                        .expect("the compiler always has a scope")
                        .insert(parameter.name.clone(), VarInfo::default());
                }

                self.task_depth += 1;
                let result = self.compile_statements(&task.body);
                self.task_depth -= 1;
                result?;

                self.emit(bc::Op::Return);
                self.emit(bc::Op::ExitScope);
                self.exit_compiler_scope();

                let end = self.ops.len();
                self.patch_jump(skip_body, end);
            }

            TaskKind::Coroutine => {
                let info = bc::CoroutineInfo { entry_point, return_type };
                self.coroutines.insert(task.name.clone(), info.clone());

                self.task_depth += 1;
                let result = self.compile_statements(&task.body);
                self.task_depth -= 1;
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
        Ok(())
    }

    fn compile_task_call(&mut self, callee: &Expression, arguments: &[Expression]) -> Result<(), MaxError> {
        if let ExpressionKind::MemberAccess { target, member } = &callee.kind {
            if let ExpressionKind::Variable(target_name) = &target.kind {
                // A variable of the same name hides any module
                let shadowed = self.is_variable_declared(target_name);

                // Native static module: math.sqrt(x). The module name is NOT pushed as `self`
                if !shadowed && self.ctx.natives.static_modules.contains(target_name) {
                    self.compile_arguments(arguments)?;
                    self.emit(bc::Op::CallNative(bc::TaskCallOperand {
                        target: Some(target_name.clone()),
                        name: member.clone(),
                        argument_count: arguments.len() as u64,
                        is_member_access: false,
                    }));
                    return Ok(());
                }

                // Imported module task: other.helper(x)
                if !shadowed
                    && let Some(table) = self.ctx.symbols.get(target_name)
                    && table.contents.contains_key(member)
                {
                    self.compile_arguments(arguments)?;
                    self.emit(bc::Op::Call(bc::TaskCallOperand {
                        target: Some(target_name.clone()),
                        name: member.clone(),
                        argument_count: arguments.len() as u64,
                        is_member_access: false,
                    }));
                    return Ok(());
                }
            }

            // Instance method: the target is pushed first, as `self`
            self.compile_expression(target)?;
            self.compile_arguments(arguments)?;
            self.emit(bc::Op::CallNative(bc::TaskCallOperand {
                target: None,
                name: member.clone(),
                argument_count: arguments.len() as u64 + 1,
                is_member_access: true,
            }));
            return Ok(());
        }

        // A plain call by name
        self.compile_arguments(arguments)?;
        let operand = self.resolve_task_operand(callee, arguments.len() as u64)?;
        self.emit(bc::Op::Call(operand));
        Ok(())
    }

    fn compile_arguments(&mut self, arguments: &[Expression]) -> Result<(), MaxError> {
        for argument in arguments {
            self.compile_expression(argument)?;
        }
        Ok(())
    }

    /// Decides whether a plain call is local or goes to an imported module
    fn resolve_task_operand(&self, callee: &Expression, argument_count: u64) -> Result<bc::TaskCallOperand, MaxError> {
        let ExpressionKind::Variable(name) = &callee.kind else {
            return Err(error(callee.span, "Only named tasks can be called."));
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
    // -------- EXPRESSIONS ------- //
    // ---------------------------- //

    fn compile_expression(&mut self, expression: &Expression) -> Result<(), MaxError> {
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

            ExpressionKind::Call { callee, arguments } => self.compile_task_call(callee, arguments)?,

            ExpressionKind::Lambda(lambda) => self.compile_lambda(lambda, expression.span)?,
        }
        Ok(())
    }

    fn compile_member_access(&mut self, target: &Expression, member: &str, span: Span) -> Result<(), MaxError> {
        let access_type = self.resolve_member_access_type(target, member, span)?;

        // A property of a native static module, e.g. math.PI. It has no `self` to push
        if access_type == bc::MemberAccessType::None
            && let ExpressionKind::Variable(name) = &target.kind
            && !self.is_variable_declared(name)
            && self.ctx.natives.static_modules.contains(name)
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

    fn compile_lambda(&mut self, lambda: &Lambda, span: Span) -> Result<(), MaxError> {
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
            child.variable_scopes[0].insert(parameter.clone(), VarInfo::default());
        }

        match &lambda.body {
            LambdaBody::Expression(expression) => {
                child.compile_expression(expression)?;
                child.emit(bc::Op::ReturnValue);
            }
            LambdaBody::Block(body) => {
                if !contains_return(body) {
                    return Err(error(span, "A block-bodied lambda must contain a return statement."));
                }
                child.compile_statements(body)?;
                child.emit(bc::Op::Return);
            }
        }

        let captured = self.collect_captures(&child.ops, &lambda.parameters);

        self.emit(bc::Op::Lambda(Arc::new(bc::LambdaOperand {
            id,
            arity: lambda.parameters.len() as u64,
            parameters: lambda.parameters.clone(),
            captured,
            chunk: Arc::new(bc::Chunk { instructions: Arc::new(child.ops) }),
        })));
        Ok(())
    }

    /// The variables a lambda body uses but does not define itself
    fn collect_captures(&self, ops: &[bc::Op], parameters: &[String]) -> Vec<String> {
        // Parameters and anything the lambda stores are its own
        let mut locals: HashSet<&str> = parameters.iter().map(String::as_str).collect();
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
}

// ---------------------------- //
// ----- FREE HELPER FUNCS ---- //
// ---------------------------- //

fn convert_modifiers(modifiers: &ModifierSet) -> bc::Modifiers {
    bc::Modifiers {
        open: modifiers.lifecycle == Some(Lifecycle::Open),
        is_static: modifiers.lifecycle == Some(Lifecycle::Static),
        fixed: modifiers.mutability == Some(Mutability::Fixed),
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
fn to_const(expression: &Expression) -> Result<bc::ConstValue, MaxError> {
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
            _ => return Err(error(expression.span, "A resort value must be a literal")),
        },

        ExpressionKind::List(items) => {
            bc::ConstValue::List(items.iter().map(to_const).collect::<Result<Vec<_>, _>>()?)
        }
        ExpressionKind::Dictionary(entries) => bc::ConstValue::Dictionary(
            entries
                .iter()
                .map(|(key, value)| Ok((to_const(key)?, to_const(value)?)))
                .collect::<Result<Vec<_>, MaxError>>()?,
        ),

        _ => return Err(error(expression.span, "A resort value must be a literal")),
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