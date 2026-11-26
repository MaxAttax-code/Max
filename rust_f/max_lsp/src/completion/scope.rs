use std::collections::HashMap;

use max_compiler::lexer::Lexer;
use max_compiler::parser::Parser;
use max_compiler::ast::{EnumMember, Expression, ExpressionKind, ModelDefinition, ModelMember, ObjectLikeDefinition, Statement, StatementKind, TypeRef};

#[derive(Clone)]
pub enum MemberTy {
    Key(&'static str),
    Enum(String),
    /// Holds a specific member known from it's path, denoted by the vector of strings that join to form said path
    EnumMember(Vec<String>),
    Object(String),
    Unknown,
}

/// A name that is visible at the cursor, with its guessed type. Variables only for now;
/// parameters will join them soon!
pub struct Symbol {
    pub name: String,
    /// The best guess to what the symbol holds *now*
    pub ty: MemberTy,
    /// What the user wrote after `as`
    pub decl_ty: MemberTy
}

/// Represents a map of enums/enum members and their respective (sub-)members
pub type EnumMembs = HashMap<String, Vec<EnumMember>>;
/// A map of model names and their definitions
pub type Objects = HashMap<String, ObjectLikeDefinition>;

/// Combines the local fields of a `Scope` into one to prevent parameter
/// overcrowding in functions
pub type ScopeLocals<'a> = (&'a EnumMembs, &'a Objects);

/// What the document defines that completion can see.
/// Do not confuse this as scopes for variable declaration!
///
/// `symbols` are **position-dependent**: meaning only declarations *before* the cursor line are included.
/// `enums` are **not** position-dependent, because the compiler hoists all enum definitions, so
/// an enum written below the cursor is still usable above it.
/// A scope held within a file
pub struct Scope {
    /// Variables and parameters, in source order
    pub symbols: Vec<Symbol>,
    /// This file's enums
    pub enums: EnumMembs,
    pub objects: Objects
}

/// A written type annotation (`as string`) to a [`MemberTy`]. Static typing means the
/// declared type is forced to be known to without checking it against the value.
pub fn ty_from_ref(t: &TypeRef, locals: ScopeLocals) -> MemberTy {
    match t {
        TypeRef::Int => MemberTy::Key("IntegerType"),
        TypeRef::Float => MemberTy::Key("FloatType"),
        TypeRef::Str => MemberTy::Key("StringType"),
        TypeRef::Boolean => MemberTy::Key("BooleanType"),
        TypeRef::Coroutine => MemberTy::Key("CoroutineType"),
        TypeRef::List(_) => MemberTy::Key("ListType"),
        TypeRef::Dictionary(..) => MemberTy::Key("DictionaryType"),
        TypeRef::Named(name) => if locals.0.contains_key(name) {
            MemberTy::Enum(name.clone())
        } else if locals.1.contains_key(name) { 
            MemberTy::Object(name.clone())
        } else { MemberTy::Unknown }
    }
}

/// Guesses the type of an expression, for declarations with no `as`
/// (`decl count = 4` means `IntegerType`).
/// 
/// The function intakes an `Expression` whose kind will be matched against,
/// the known `Symbol`s within the scope so far, and a map of all enums
/// within the `Scope`; Recall that enums are global in a file.
///
/// It follows the same rules as the VM's arithmetic: a string on either side of `+` gives a
/// string, an int mixed with a float gives a float, comparisons and `and`/`or` give booleans, and
/// a plain variable takes on that variable's type (searched backwards so shadowing works)... And so on.
/// Calls, indexing and anything else are `Unknown`.
fn infer(expr: &Expression, known: &[Symbol], locals: ScopeLocals) -> MemberTy {
    match &expr.kind {
        ExpressionKind::Integer(_) => MemberTy::Key("IntegerType"),
        ExpressionKind::Float(_) => MemberTy::Key("FloatType"),
        ExpressionKind::Str(_) => MemberTy::Key("StringType"),
        ExpressionKind::Boolean(_) => MemberTy::Key("BooleanType"),
        ExpressionKind::List(_) => MemberTy::Key("ListType"),
        ExpressionKind::Dictionary(_) => MemberTy::Key("DictionaryType"),

        // `declare b = a` takes a's type, as simple as pi!
        ExpressionKind::Variable(name) => {
            if let Some(found_var) = known.iter().rev().find(|s| &s.name == name) {
                found_var.ty.clone()
            }
            else if let Some((enum_name, _)) = locals.0.iter().find(|(e, _)| *e == name) {
                MemberTy::Enum(enum_name.clone())
            }
            else {
                MemberTy::Unknown
            }
        },

        ExpressionKind::Comparison { .. } | ExpressionKind::Logical { .. } => MemberTy::Key("BooleanType"),

        ExpressionKind::Binary { left, right, .. } => match (infer(left, known, locals), infer(right, known, locals)) {
            (MemberTy::Key("StringType"), _) | (_, MemberTy::Key("StringType")) => MemberTy::Key("StringType"),

            (MemberTy::Key("FloatType"), MemberTy::Key("IntegerType" | "FloatType"))

            | (MemberTy::Key("IntegerType"), MemberTy::Key("FloatType")) => MemberTy::Key("FloatType"),

            (MemberTy::Key("IntegerType"), MemberTy::Key("IntegerType")) => MemberTy::Key("IntegerType"),
            _ => MemberTy::Unknown,
        },

        ExpressionKind::MemberAccess { .. } => {
            // Get the chain or get out!
            let Some(chain) = flatten_chain(expr) else { return MemberTy::Unknown; };

            // Declared vars always come first!
            let mut path = match known.iter().rev().find(|s| s.name == chain[0]) {
                Some(Symbol {
                    // Enum member? Nice!
                    ty: MemberTy::EnumMember(p), ..
                }) => p.clone(),
                Some(Symbol {
                    ty: MemberTy::Object(o), ..
                }) => vec![o.clone()],

                // Anything else => IDK!
                Some(_) => return MemberTy::Unknown,

                None => vec![chain[0].clone()]
            };
            // Always extend the rest in
            path.extend(chain[1..].iter().cloned());

            // It's *atleast* one member accessing, and enum children were found from the path
            if path.len() >= 2 && enum_children(&locals.0, &path).is_some() {
                MemberTy::EnumMember(path)
            } else {
                // What else
                MemberTy::Unknown
            }
        }

        _ => MemberTy::Unknown,
    }
}

/// The members reachable after `path`, e.g. `["Item", "Tool"]` gives Tool's sub-members.
/// `None` if any segment doesn't exist. `Some(empty)` means a real member with no children.
pub fn enum_children<'a>(enums: &'a EnumMembs, path: &[String]) -> Option<&'a [EnumMember]> {
    let mut members = enums.get(path.first()?)?.as_slice();
    for segment in &path[1..] {
        let member = members.iter().find(|m| &m.name == segment)?;
        members = match &member.sub_enum_reference {
            Some(reference) => enums.get(reference)?.as_slice(),
            // No sub-reference = look for sub-members
            None => member.sub_members.as_slice(),
        };
    }
    Some(members)
}

/// Gets the member found at the end of the given path. This utilises `Option`
/// to more easily handle errors and getting items.
pub fn object_members<'a>(objects: &'a Objects, path: &[String]) -> Option<&'a [ModelMember]> {
    let model_members = objects.get(path.first()?)?.members.as_slice();
    Some(model_members)
}

/// Flattens a given Expression of Member accessing into a vector of strings. A single variable
/// expression always results in a Vector of one item.
/// ### Visualisation
/// `a.b.c` as `["a", "b", "c"]`; `None` if the expression is anything other than names and dots
/// 
/// `d` as `["d"]`; `d` is a variable and considered a chain of length one
fn flatten_chain(expr: &Expression) -> Option<Vec<String>> {
    match &expr.kind {
        ExpressionKind::Variable(name) => Some(vec![name.clone()]),
        
        ExpressionKind::MemberAccess { target, member } => {
            let mut chain = flatten_chain(target)?;
            chain.push(member.clone());
            Some(chain)
        }

        _ => None,
    }
}

/**
Walks `statements` and pushes a [`Symbol`] for every declaration that starts on or before
`cursor_line`, going down into the bodies of `if`, loops and tasks. As it does, it
collects from the given statements and the cursor line, updating the `Vec<Symbol>` inputted in.

This ignores all statements behind the `cursor_line` (using their span start lines),
Statements are in source order, so the first one past the cursor ends the walk (`break`).

then searches for each statements kind to match and create valid Symbols to push in.
This can recursively call itself when going through loop/statement bodies.

**Known limitation:** scoping isn't tracked. A variable declared inside a block that has
already ended still shows up, because we don't know where blocks end (statement spans cover
only the first token). That will need whole-statement spans.
*/
fn collect(statements: &[Statement], locals: ScopeLocals, cursor_line: u32, out: &mut Vec<Symbol>) {
    for statement in statements {
        if statement.span.start_line > cursor_line { break; }
        match &statement.kind {
            StatementKind::Declaration(d) => {
                // First we infer
                let inferred = infer(&d.value, out, locals);
                // then get what *could* be the declared type
                let declared = d.type_ref.as_ref().map_or(MemberTy::Unknown, |r| ty_from_ref(&r.node, locals));

                // and match
                let ty = match (&declared, inferred) {
                    /*
                    WOOOAH! What's '@' doing here?
                    The '@' symbol simply allows us to bind a value during pattern matching.
                    In a way, similar to C#!
                    */
                    (_, t @ MemberTy::EnumMember(_)) => t,


                    // Only inferred
                    (MemberTy::Unknown, inferred) => inferred,

                    // Known declaration
                    (declared, _) => declared.clone(),
                };

                out.push(Symbol { name: d.name.clone(), ty, decl_ty: declared });
            }

            StatementKind::If(i) => {
                collect(&i.first.body, locals, cursor_line, out);
                for e in &i.elifs { collect(&e.body, locals, cursor_line, out); }
                collect(&i.else_block, locals, cursor_line, out);
                collect(&i.maybe_block, locals, cursor_line, out);
            }

            StatementKind::While(w) => collect(&w.body, locals, cursor_line, out),

            StatementKind::For(f) => collect(&f.body, locals, cursor_line, out),

            StatementKind::Foreach(fe) => collect(&fe.body, locals, cursor_line, out),

            StatementKind::Match(ma) => {
                for case in ma.cases.iter() {
                    collect(&case.body, locals, cursor_line, out)
                }
            }

            StatementKind::TaskDefinition(t) => collect(&t.body, locals, cursor_line, out),

            _ => {}
        }
    }
}

/// Builds the [`Scope`] for a document, using a [`Lexer`] to provide the statements to collect.
///
/// `cursor_line` is **1-based**, to match the spans in the AST; callers convert from LSP's
/// 0-based lines with `+ 1`.
///
/// It lexes and parses the whole document on every call. The parser's error recovery returns
/// whatever statements it managed to read, so a half-typed line costs only itself. But a
/// **lexer** error (unterminated string or block comment) gives an empty scope, because
/// tokenising currently stops at the first error.
/// 
/// Self Note: *Caching by document version is the obvious
/// optimisation if this ever feels slow.*
pub fn build(text: &str, cursor_line: u32) -> Scope {
    let mut scope = Scope { symbols: Vec::new(), enums: HashMap::new(), objects: HashMap::new() };

    // A lexer error anywhere (unterminated string/comment) means no scope for now
    let Ok(tokens) = Lexer::new(text).tokenise() else { return scope };
    let (statements, _diagnostics) = Parser::new(tokens).parse();

    for s in &statements {
        match &s.kind {
            StatementKind::Enum(e) => { scope.enums.insert(e.identifier.clone(), e.members.clone()); }
            StatementKind::Model(m) => {
                match m {
                    ModelDefinition::ObjectLike(obj) => { scope.objects.insert(obj.name.node.clone(), obj.clone()); }
                }
            }

            _ => {}
        }
    }
    collect(&statements, (&scope.enums, &scope.objects), cursor_line, &mut scope.symbols);
    scope
}