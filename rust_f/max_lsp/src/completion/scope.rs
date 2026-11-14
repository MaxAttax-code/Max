use std::collections::HashMap;

use max_compiler::lexer::Lexer;
use max_compiler::parser::Parser;
use max_compiler::ast::{EnumMember, Expression, ExpressionKind, Statement, StatementKind, TypeRef};

#[derive(Clone)]
pub enum MemberTy {
    Key(&'static str),
    #[allow(dead_code)]
    Enum(String),
    Unknown,
}

/// For accessing a member
pub struct Symbol {
    pub name: String,
    pub ty: MemberTy
}

/// A scope held within a file
pub struct Scope {
    /// Variables and parameters, in source order
    pub symbols: Vec<Symbol>,
    /// This file's enums
    pub enums: HashMap<String, Vec<EnumMember>>, 
}

pub fn ty_from_ref(t: &TypeRef) -> MemberTy {
    match t {
        TypeRef::Int => MemberTy::Key("IntegerType"),
        TypeRef::Float => MemberTy::Key("FloatType"),
        TypeRef::Str => MemberTy::Key("StringType"),
        TypeRef::Boolean => MemberTy::Key("BooleanType"),
        TypeRef::Coroutine => MemberTy::Key("CoroutineType"),
        TypeRef::List(_) => MemberTy::Key("ListType"),
        TypeRef::Dictionary(..) => MemberTy::Key("DictionaryType"),
        TypeRef::Named(name) => MemberTy::Enum(name.clone()),
    }
}

fn infer(expr: &Expression, known: &[Symbol]) -> MemberTy {
    match &expr.kind {
        ExpressionKind::Integer(_) => MemberTy::Key("IntegerType"),
        ExpressionKind::Float(_) => MemberTy::Key("FloatType"),
        ExpressionKind::Str(_) => MemberTy::Key("StringType"),
        ExpressionKind::Boolean(_) => MemberTy::Key("BooleanType"),
        ExpressionKind::List(_) => MemberTy::Key("ListType"),
        ExpressionKind::Dictionary(_) => MemberTy::Key("DictionaryType"),

        // `declare b = a` takes a's type, as simple as pi!
        ExpressionKind::Variable(name) => known.iter().rev().find(|s| &s.name == name)
            .map_or(MemberTy::Unknown, |s| s.ty.clone()),

        ExpressionKind::Comparison { .. } | ExpressionKind::Logical { .. } => MemberTy::Key("BooleanType"),

        ExpressionKind::Binary { left, right, .. } => match (infer(left, known), infer(right, known)) {
            (MemberTy::Key("StringType"), _) | (_, MemberTy::Key("StringType")) => MemberTy::Key("StringType"),

            (MemberTy::Key("FloatType"), MemberTy::Key("IntegerType" | "FloatType"))

            | (MemberTy::Key("IntegerType"), MemberTy::Key("FloatType")) => MemberTy::Key("FloatType"),

            (MemberTy::Key("IntegerType"), MemberTy::Key("IntegerType")) => MemberTy::Key("IntegerType"),
            _ => MemberTy::Unknown,
        },
        _ => MemberTy::Unknown,
    }
}

/**
Collects from the given statements and the cursor line, updating the `Vec<Symbol>` inputted in.
This ignores all statements behind the `cursor_line` (using their span start lines),
then searches for each statements kind to match and create valid Symbols to push in.
This can recursively call itself when going through loop/statement bodies.
*/
fn collect(statements: &[Statement], cursor_line: u32, out: &mut Vec<Symbol>) {
    for statement in statements {
        if statement.span.start_line > cursor_line { break; }
        match &statement.kind {
            StatementKind::Declaration(d) => {
                let ty = match &d.type_ref {
                    Some(t) => ty_from_ref(&t.node),
                    None => infer(&d.value, out),        // step 4
                };
                out.push(Symbol { name: d.name.clone(), ty });
            }

            StatementKind::If(i) => {
                collect(&i.first.body, cursor_line, out);
                for e in &i.elifs { collect(&e.body, cursor_line, out); }
                collect(&i.else_block, cursor_line, out);
                collect(&i.maybe_block, cursor_line, out);
            }

            StatementKind::While(w) => collect(&w.body, cursor_line, out),

            StatementKind::For(f) => collect(&f.body, cursor_line, out),

            StatementKind::Foreach(f) => collect(&f.body, cursor_line, out),

            StatementKind::TaskDefinition(t) => collect(&t.body, cursor_line, out),

            _ => {}
        }
    }
}

/// Builds from the given text and the current cursor line. This calls the lexer to tokenise the text,
/// then parses through the lexed text. The statements are filtered to add specific fields to the [`Scope`],
/// then all statements are collected with [`collect`]. The final scope is returned.
pub fn build(text: &str, cursor_line: u32) -> Scope {
    let mut scope = Scope { symbols: Vec::new(), enums: HashMap::new() };

    // A lexer error anywhere (unterminated string/comment) means no scope for now
    let Ok(tokens) = Lexer::new(text).tokenise() else { return scope };
    let (statements, _diagnostics) = Parser::new(tokens).parse();

    for s in &statements {
        if let StatementKind::Enum(e) = &s.kind {
            scope.enums.insert(e.identifier.clone(), e.members.clone());
        }
    }
    collect(&statements, cursor_line, &mut scope.symbols);
    scope
}