use max_bytecode::MaxType;
use max_compiler::ast::{ModelMember, ModelMemberKind};
use max_compiler::{ast::EnumMember, lexer::Lexer};
use max_compiler::token::{Token, TokenKind as K};
use max_runtime::natives::{MemberInfo, NativeRegistry, ParamSpec};
use super::scope::*;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, Position};

/// Returns the part of the cursor's line **before** the cursor.
///
/// The LSP `character` is a count of UTF-16 code units, but a Rust `&str` is sliced by bytes.
/// So we walk the line one `char` at a time, adding each char's UTF-16 length, until we've passed
/// `pos.character` units, and cut at that char's byte index. That keeps emoji and other
/// non-ASCII characters from landing the cut in the wrong place (or in the middle of a character,
/// which would panic!).
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

/// Represents what kind of Value Targetting will be performed in the `Context::Value`
pub enum ValueTarget {
    /// Declaration (decl x as ... =)
    Declared(String),
    /// `x =`, for looking up the variable
    Variable(String)
}

/// What kind of thing is being typed at the cursor.
///
/// This decides *which list* to offer. It doesn't decide what's in the list.
pub enum Context {
    Member(Vec<String>),
    TypeName,
    TaskKind,
    ModelKind,
    Value(ValueTarget),
    General,
    None
}

/**
Decides the [`Context`] for the text before the cursor.

## How it works
The prefix is lexed (using the real lexer, so strings and comments are handled correctly),
then we look at the **last complete token**:

- The word being typed is dropped first. `x.le|` lexes as `x`, `.`, `le`, and the half-typed
  `le` is an `Identifier`, so it's ignored. That's also why `x.|` and `x.le|` give the same
  answer.
- `.` leads to a `Member` chain. We walk backwards over `name . name . name`, collecting the
  names, then reverse them. For `a.b.|` the chain is `["a", "b"]`.
- `as` or `of` gives `TypeName`; `define` gives `TaskKind`; anything else is `General`.

## When it returns `None`
- The lexer fails: most often an **unterminated string**, so the cursor is inside it.
- The chain is empty, e.g. `foo().|` or `list[0].|`, where the thing before the dot isn't a
  plain name. We can't resolve those yet, so we offer nothing, not a wrong list.

## Known limits
- A half-typed word that is *also a keyword* (`x.to|`, `as in|`) lexes as a keyword, not an
  `Identifier`, so it is not dropped and the context is wrong.
- The last identifier is dropped even if there is a space after it (`define native |`), as if
  it were still being typed.
*/
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
        Some(K::Model) => Context::ModelKind,

        // Everything before the '='
        Some(K::Equals) => value_context(&tokens[..end - 1]),

        // Also create Value suggestions for comparing things
        Some(K::Comparator | K::NotEqualTo) => match &tokens[..end - 1] {
            [.., name] if name.kind == K::Identifier => Context::Value(ValueTarget::Variable(name.value.clone())),
            _ => Context::General
        }

        _ => Context::General,
    }
}

/**
Creates `Context` from the given tokens. This is used for giving
context on providing a value, and depends on the kind of assigning to be done.

With a declaration, the `decl` is found, and **modifiers are skipped**, capturing
only the name and the type assigned if so. A `Context::Value` is returned with
it's inner `ValueTarget::Declared` holding the type annotations name.
Without type annotations, this returns `Context::General`.

With plain assignment ('='), the statement can be find on it's own (`name =`) or
following a statement starter (e.g. `then name =`). In both cases, a `Context::Value`
is returned with it's `ValueTarget::Variable` holding the identifier name. Said name
will be used to *lookup* the variable in the `Scope`.
*/
fn value_context(before: &[Token]) -> Context {
    // Declaration: `decl [modifiers] name [as Type] =`
    if let Some(d) = before.iter().rposition(|t| t.kind == K::Decl) {
        // Iterate over what's after the decl
        let rest: Vec<&Token> = before[d + 1..].iter()
            // Skip the modifiers
            .skip_while(|t| t.kind == K::Modifier)
            .collect();

        return match rest.as_slice() {
            [name, as_token, ty]
                if name.kind == K::Identifier && as_token.kind == K::As && ty.kind == K::Identifier =>
                    Context::Value(ValueTarget::Declared(ty.value.clone())),
            
            // No annotation, nothing to go by
            _ => Context::General,
        };
    }

    // Plain assignment: `name =` as a statement of its own
    match before {
        [name] if name.kind == K::Identifier =>
            Context::Value(ValueTarget::Variable(name.value.clone())),

        [.., prev, name]
            if name.kind == K::Identifier && starts_statement(prev.kind) =>
                Context::Value(ValueTarget::Variable(name.value.clone())),

        _ => Context::General,
    }
}

/// Tokens after which a new statement can begin on the same line
/// (`if c then x = `, `else x = `, ...)
/// 
/// This is specifically used for [`value_context`] in order to identify assignments
/// that follow right after a statement starter.
fn starts_statement(kind: K) -> bool {
    matches!(kind, K::Then | K::Do | K::Else | K::Maybe | K::ConcludeStatement | K::Semicolon)
}

/**
Completes `chain.|`, where `chain[0]` is a name and the rest are members, and returns the
members of the type the chain ends on.

## Resolution
1. Find `chain[0]` among the scope's symbols. The search runs **backwards**, so the latest
   declaration wins when a name has been shadowed with `open`, therefore that declarations
   info is what is relied on.
2. Follow each remaining segment with [`member_type`]: `s.first.` asks "what type does
   `first` return?" and continues from there.
3. If the chain ends on a type that has native members, list them via
   [`NativeRegistry::members_of`]. This is always a fixed list from the `NativeRegistry`
   based on the ending type

Whenever a step can't be resolved, the type becomes [`MemberTy::Unknown`] and the result is
empty. We return nothing rather than guess.

Not handled yet: native modules (`math.`) and user modules (`util.`). Those
are separate branches that go before the final `vec![]`.
*/
pub fn complete_member(chain: &[String], scope: &Scope, registry: &NativeRegistry) -> Vec<CompletionItem> {
    if let Some(sym) = scope.symbols.iter().rev().find(|s| s.name == chain[0]) {
        let mut ty = sym.ty.clone();
        for segment in &chain[1..] {
            ty = member_type(&ty, segment, registry, (&scope.enums, &scope.objects));
        }

        return match ty {
            MemberTy::Key(key) => registry.members_of(key).iter().map(member_item).collect(),
            _ => vec![],
        };
    }

    // Infants? I couldn't think of anything else haha
    if let Some(enum_infants) = enum_children(&scope.enums, chain) {
        let items = enum_members(enum_infants);
        return items;
    }

    if let Some(object_members) = object_members(&scope.objects, chain) {
        let items = object_completions(object_members);
        return items;
    }
    
    vec![]
}

/// Creates `CompletionItem`s out of the given enum members.
/// 
/// Each member is labelled with it's name and it's enumerable value.Should the member
/// have any `sub_members`, the count is written in details, or in the case of
/// `sub_enum_reference`, the referenced enum's name. The `CompletionItemKind` is
/// always `CompletionItemKind::ENUM_MEMBER`
fn enum_members(enum_members: &[EnumMember]) -> Vec<CompletionItem> {
    enum_members.iter().map(|member| {
        let mut detail = format!("Enum-member: {}\nEnumerable: {}", member.name, member.enumerable);

        if !member.sub_members.is_empty() {
            detail += &format!("\nSub-members count: {}", member.sub_members.len());
        } else if let Some(sub_ref) = &member.sub_enum_reference {
            detail += &format!("\nSub-member reference: {sub_ref}");
        }

        CompletionItem {
            label: member.name.clone(),
            detail: Some(detail),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            ..Default::default()
        }
    }).collect()
}

fn object_completions(object_members: &[ModelMember]) -> Vec<CompletionItem> {
    object_members.iter().map(|member| {
        match member {
            ModelMember::Field(f) => {
                let detail =
                    format!("Field '{}'",
                        f.declaration.name
                    );

                let mut documentation = f.attributes.markdown();

                documentation += "\n- **Modifiers**:";

                // If there is not modifier found, use 'Modifiers: None'
                let mut has_modifier = false;
                if let Some(mutable) = f.declaration.modifiers.mutability {
                    use max_compiler::ast::Mutability;
                    let name = match mutable {
                        Mutability::Fixed => "fixed"
                    };
                    documentation += &format!(" {}", name);
                    has_modifier = true;
                }
                if let Some(lifecycle) = f.declaration.modifiers.lifecycle {
                    use max_compiler::ast::Lifecycle;
                    let name = match lifecycle {
                        Lifecycle::Monic => "monic",
                        Lifecycle::Open => "open"
                    };
                    documentation += &format!(" {}", name);
                    has_modifier = true;
                }
                
                if !has_modifier {
                    documentation += " None";
                }

                CompletionItem {
                    label: f.to_name(),
                    detail: Some(detail),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: documentation
                    })),
                    kind: Some(CompletionItemKind::FIELD),
                    ..Default::default()
                }
            },
            ModelMember::Task(t) => {
                let detail =
                    format!("Task '{}'",
                        t.definition.name
                    );

                let mut documentation = t.attributes.markdown();

                // Extra extra detail!
                documentation += &format!("\n- **Task Kind**: {}", t.definition.kind);

                CompletionItem {
                    label: t.to_name(),
                    detail: Some(detail),
                    kind: Some(CompletionItemKind::METHOD),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: documentation
                    })),
                    ..Default::default()
                }
            }
        }
    }).collect()
}

/// One hop along a chain: given the type we're on and a member name, returns the **type that
/// member produces**.
///
/// It finds the member in the registry and converts its declared return type to a registry key.
/// Enum members are handled by [`enum_children`], and specify their path by pushing the given
/// name into the existing member path (this new path is used to resolve the children).
/// A member with no declared return type (`list.first`), or one that returns something with no
/// members of its own (`NumberType`), gives `Unknown`.
fn member_type(ty: &MemberTy, name: &str, registry: &NativeRegistry,
               locals: ScopeLocals) -> MemberTy {
    match ty {
        MemberTy::Key(key) =>
            registry.members_of(key).into_iter()
                .find(|m| m.name == name)
                .and_then(|m| m.return_type)
                // Get that name!!!
                .and_then(|t| type_key(&t))
                .map_or(MemberTy::Unknown, MemberTy::Key),

        // Extend if the member exists
        MemberTy::EnumMember(path) => {
            let mut next = path.clone();
            // Push the given name
            next.push(name.to_owned());
            // And find the (possible) enum children
            if enum_children(locals.0, &next).is_some() {
                MemberTy::EnumMember(next)
            } else {
                MemberTy::Unknown
            }
        }

        MemberTy::Object(obj_name) => {
            let members: &Vec<ModelMember> = locals.1.iter()
                .find(|obj| obj.0 == obj_name)
                .and_then(|o| Some(&o.1.members))
                .expect(&format!("No object by the name of '{}' was found", obj_name))
            ;

            let member: MemberTy = members.iter().find_map(|mem| {
                match mem {
                    ModelMember::Field(field) if field.declaration.name == name
                        => Some(field.declaration.name.clone()),
                    ModelMember::Task(task) if task.definition.name == name
                        => Some(task.definition.name.clone()),

                    _ => None
                }
            })
            // Map the found member name into the MemberTy
            .map_or(MemberTy::Unknown, MemberTy::Object);

            member
        }

        _ => MemberTy::Unknown
    }
    
}

/// Converts a native member into a completion item.
///
/// - `detail` is the signature, e.g. `(string, string) -> string`.
/// - Methods insert `name()` with the cursor placed between the parentheses (`$0` is the snippet's
///   final cursor position). With no parameters, the cursor goes after the `()`.
/// - Properties (`list.length`) insert just the name, with no parentheses.
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

/// Converts a [`MaxType`] to the **registry key** its members are registered under, such as
/// `StringType` or `ListType`. These strings must match the `NativeTarget::Type("...")` names
/// used when registering natives; if they differ, a type quietly gets no completions.
///
/// Returns `None` for types with no native members: `NumberType`, `PossibleType`, `NoType`,
/// enums and lambdas.
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
        MaxType::ObjectType(obj_name) => obj_name.clone(),
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