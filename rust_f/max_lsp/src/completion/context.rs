use max_bytecode::MaxType;
use max_compiler::lexer::Lexer;
use max_compiler::token::TokenKind as K;
use max_runtime::natives::{MemberInfo, NativeRegistry, ParamSpec};
use super::scope::*;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, Position};

/// Gets the line prefix of the position within the `text`. This checks through
/// the position's line and character to pinpoint the slice of the line to return
pub fn line_prefix(text: &str, pos: Position) -> &str {
    let line = text.lines().nth(pos.line as usize).unwrap_or("");
    let mut units = 0;
    // Go through each char in the line
    for (byte, ch) in line.char_indices() {
        // This returns if the position is before the end of the line
        if units >= pos.character as usize { return &line[..byte]; }
        units += ch.len_utf16();
    }
    line
}

pub enum Context {
    Member(Vec<String>),
    TypeName,
    TaskKind,
    General,
    None
}

/// Provides context to the given prefix by ensuring it is an unfinished identifier, as well
/// as checking other factors such as dots for member accessing. This then returns a final `Context`
pub fn context(prefix: &str) -> Context {
    // An unterminated string means the cursor is inside one: offer nothing
    let Ok(tokens) = Lexer::new(prefix).tokenise() else { return Context::None };

    let mut end = tokens.len();
    // It would be an identifier since it is not completed to the word it should be
    if end > 0 && tokens[end - 1].kind == K::Identifier { end -= 1 }

    match end.checked_sub(1).map(|i| tokens[i].kind) {
        Some(K::Dot) => {
            // Walk backwards over `a.b.c`
            end -= 1;

            let mut chain = Vec::new();
            while end > 0 && tokens[end - 1].kind == K::Identifier {
                chain.push(tokens[end - 1].value.clone());
                end -= 1;
                if end > 0 && tokens[end - 1].kind == K::Dot { end -= 1 } else { break }
            }
            chain.reverse();
            // Member context through the accessors
            if chain.is_empty() { Context::None } else { Context::Member(chain) }
        }

        // These are more simple...
        Some(K::As) | Some(K::Of) => Context::TypeName,

        Some(K::Def) => Context::TaskKind,

        _ => Context::General,
    }
}

/// First finds the name corresponding to the chains first item, then looks up the MemberType
/// and creates a member name out of said type. This then returns the valid members of the given
/// member item, otherwise nothing.
pub fn complete_member(chain: &[String], scope: &Scope, registry: &NativeRegistry) -> Vec<CompletionItem> {
    if let Some(sym) = scope.symbols.iter().rev().find(|s| s.name == chain[0]) {
        let mut ty = sym.ty.clone();
        for segment in &chain[1..] {
            ty = member_type(&ty, segment, registry);
        }

        return match ty {
            MemberTy::Key(key) => registry.members_of(key).iter().map(member_item).collect(),
            _ => vec![],
        };
    }
    vec![]
}

/// Creates a `MemberTy` by finding the correct member name, then linking it to
/// it's type key (see [`type_key`]) and returning as a Member type
fn member_type(ty: &MemberTy, name: &str, registry: &NativeRegistry) -> MemberTy {
    let MemberTy::Key(key) = ty else { return MemberTy::Unknown };
    registry.members_of(key).into_iter()
        .find(|m| m.name == name)
        .and_then(|m| m.return_type)
        // Get that name!!!
        .and_then(|t| type_key(&t))
        .map_or(MemberTy::Unknown, MemberTy::Key)
}

/// Creates a `CompletionItem` out of the referenced [`MemberInfo`]
pub fn member_item(m: &MemberInfo) -> CompletionItem {
    CompletionItem {
        label: m.name.clone(),
        kind: Some(if m.is_property { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }),
        detail: Some(signature(m)),
        documentation: (!m.doc.is_empty()).then(|| Documentation::String(m.doc.into())),
        insert_text: (!m.is_property).then(|| {
            if m.params.is_empty() { format!("{}()", m.name) } else { format!("{}($0)", m.name) }
        }),
        insert_text_format: Some(InsertTextFormat::SNIPPET),   // `$0` puts the cursor inside the parentheses
        ..Default::default()
    }
}

/// MaxType -> the registry's target key, for looking up that type's members
fn type_key(t: &MaxType) -> Option<&'static str> {
    Some(match t {
        MaxType::IntegerType => "IntegerType",
        MaxType::FloatType => "FloatType",
        MaxType::StringType => "StringType",
        MaxType::BooleanType => "BooleanType",
        MaxType::ListType(_) => "ListType",
        MaxType::DictionaryType(..) => "DictionaryType",
        MaxType::CoroutineType => "CoroutineType",
        _ => return None,   // Number, Possible, NoType, enums, tasks: no members to offer
    })
}

/// How a type is written in Max source, not the internal `{name}Type` names
fn type_name(t: &MaxType) -> String {
    match t {
        MaxType::NumberType => "number".into(),
        MaxType::IntegerType => "int".into(),
        MaxType::FloatType => "float".into(),
        MaxType::StringType => "string".into(),
        MaxType::BooleanType => "boolean".into(),
        MaxType::PossibleType => "possible".into(),

        MaxType::ListType(None) => "list".into(),
        // Lists and dictionaries that have element types show it in the short form
        MaxType::ListType(Some(e)) => format!("list<{}>", type_name(e)),
        MaxType::DictionaryType(Some(k), Some(v)) => format!("dictionary<{}, {}>", type_name(k), type_name(v)),
        MaxType::DictionaryType(..) => "dictionary".into(),

        // These aren't exactly true but...
        MaxType::CoroutineType => "coroutine".into(),
        MaxType::TaskType(_) => "lambda".into(),
        MaxType::EnumType(name) => name.clone(),
        MaxType::NoType => "any".into(),
    }
}

/// Provides the signature of a task or property, where the intaken parameters are
/// wrapped in brackets ('?' for Optionals and '...' for Variadics), and display
/// the return type following.\
/// e.g. `(string, string) -> string`, or just `-> int` for properties
fn signature(member: &MemberInfo) -> String {
    // Get the return type
    let ret = member.return_type.as_ref().map_or("any".to_string(), type_name);
    if member.is_property {
        return format!("-> {ret}");
    }
    // Each parameter is uniquely signed
    let params: Vec<String> = member.params.iter().map(|p| match p {
        ParamSpec::Required(t) => t.as_ref().map_or("any".into(), type_name),
        ParamSpec::Optional(t) => format!("{}?", t.as_ref().map_or("any".into(), type_name)),
        ParamSpec::Variadic(t) => format!("{}...", t.as_ref().map_or("any".into(), type_name)),
    }).collect();
    // The final result is exactly as expected
    format!("({}) -> {ret}", params.join(", "))
}