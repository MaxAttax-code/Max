use std::sync::LazyLock;

use max_runtime::natives::NativeRegistry;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position};
use crate::completion::context::{Context, context, line_prefix, complete_member};

mod context;
mod scope;

/// Creates a completion item from the given label, detail and kind ([`CompletionItemKind`]).
/// If the detail is an empty String after `into()`, it is regarded as None in the `CompletionItem` creation
fn create_completion_item(label: impl Into<String>, detail: impl Into<String>, kind: CompletionItemKind) -> CompletionItem {
    let detail = detail.into();
    let detail: Option<String> = if detail.is_empty() {
        None
    } else { Some(detail) };

    CompletionItem {
        label: label.into(), detail, kind: Some(kind),
        ..Default::default()
    }
}

/// Creates a vector of `CompletionItem`s using a given vector of (label, detail, kind)
fn from_comp_items(items: Vec<(impl Into<String>, impl Into<String>, CompletionItemKind)>) -> Vec<CompletionItem> {
    let mut completion_items: Vec<CompletionItem> = Vec::new();
    for (label, detail, kind) in items {
        completion_items.push(create_completion_item(label, detail, kind));
    }
    completion_items
}

/// All the defined types in Max with their label and details are stored here
static TYPE_NAME_ITEMS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    from_comp_items(vec![
        ("int", "An integer in Max, up to 64-bit and signed", CompletionItemKind::TYPE_PARAMETER),
        ("float", "A float in Max, up to 64-bit and signed", CompletionItemKind::TYPE_PARAMETER),
        ("string", "A string max in Max, indicated by double quotations. Almost all types can be transformed into a string representation", CompletionItemKind::TYPE_PARAMETER),
        ("boolean", "A boolean in Max, can be either true, false, or possible", CompletionItemKind::TYPE_PARAMETER),
        ("list", "A collection of other types in one type. This can hold an element type", CompletionItemKind::TYPE_PARAMETER),
        ("dictionary", "A collection of keys and values, each being their own individual type", CompletionItemKind::TYPE_PARAMETER),
    ])
});

/// All the defined task types in Max, they are regarded as Type parameters as well
static TASK_TYPE_NAME_ITEMS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    from_comp_items(vec![
        ("function", "A function task that can be called and passed arguments into", CompletionItemKind::TYPE_PARAMETER),
        ("coroutine", "A coroutine task that can be called run, pause, cancel, or terminate on, and can yield values", CompletionItemKind::TYPE_PARAMETER),
        ("event", "An event task that can be created in multiple partials, and emitted to call all partials in order", CompletionItemKind::TYPE_PARAMETER),
        ("native", "A keyword used to define a native Max object", CompletionItemKind::TYPE_PARAMETER),
    ])
});

/// All the general items that can appear from a normal completion action
static GENERAL_NAME_ITEMS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    from_comp_items(vec![
        (
            "using",
            "The 'using' keyword for Max. Use this to import a module from a library or your own modules. \
            This can also import objects and symbols from a module using member accessing (e.g. using math.cos). \
            Always end the using statement with a semicolon. It guarantees the server to know what you are using/ \
            The word 'using' is a real consideration as to how importing in Max works, as 'using' a module means \
            the compiler has automatic access to whatever was imported, essentially knowing what was imported in \
            past tense. This means the Runtime cares not about what methods or objects imported are passed into it, \
            it only cares that the imported object exists from the compiler's knowledge.",
            CompletionItemKind::KEYWORD
        ),
        (
            "decl",
            "Begins a variable declaration. Declarations in Max can be verbose but straightforward, as the structure is as follows:
             - 'decl' -> [modifiers (e.g. 'fixed')] -> name -> [as type] -> = value -> [, resort otherValue]
            Although the creator of Max used PascalCase in his variable naming, there is no strict casing guidelines for declarations.",
            CompletionItemKind::KEYWORD
        ),
        (
            "resort",
            "The keyword in a declaration that declares a variable's resort value. A resort value is a held value within a declaration \
            that is called when the variable becomes nil, and immediately replaces the nil with the resort value. Essentially, a declaration \
            with a resort value can never be nil. Resort values can never be changed, and also follow typing rules if specified in the declaration.",
            CompletionItemKind::KEYWORD
        ),
        ("say", "Max's built-in keyword to print out whatever follows it to the stdout", CompletionItemKind::KEYWORD),
        (
            "def",
            "Begins a task definition. Task definitions in Max are symbols in a module that vary in concept and use, but \
            always follow two core rules: They can be called in one way or another, and they hold repeatable code. \
            Task definitions are always able to imported from other modules, and are global in the module once defined.",
            CompletionItemKind::KEYWORD
        ),
        (
            "enum",
            "Begins an enum definition. Enums are containers that hold members that can be accessed from the container. \
            Enums are global to the entire module once defined (unless it is skipped over in runtime), and are accessible \
            from other modules. Enum members can hold sub-members or a sub-enum reference. These essentially allow members of \
            an enum to be containers themselves that hold enums deeper in the 'roots' of an enum.",
            CompletionItemKind::KEYWORD
        ),
        ("open", "A declaration modifier that allows a variable to be shadowed. \
        A variable that shadows another variable can also be declared as open.", CompletionItemKind::KEYWORD),
        ("static", "A declaration modifier that ensures a variable to have one single represented value across all modules.", CompletionItemKind::KEYWORD),
        ("fixed", "A declaration modifier that fixes a variables value to whatever was first declared. \
        A fixed declaration MUST be initialised with a value.", CompletionItemKind::KEYWORD),
        ("if", "Allows evaluating a condition and running different blocks of code depending on the condition result. \
        The condition must always result in a boolean.", CompletionItemKind::KEYWORD),
        ("then", "Starts a new block of code. This is identical to 'do', but better represents the statements it is used for.", CompletionItemKind::KEYWORD),
        ("elif", "Adds another conditional branch to be evaluated in an if statement.", CompletionItemKind::KEYWORD),
        ("else", "Represents the start of a block of code that is defaulted to when a condition is not met.", CompletionItemKind::KEYWORD),
        ("maybe", "A specific block of code for if statements that is evaluated when the condition is a possible.", CompletionItemKind::KEYWORD),
        ("conclude", "A universal keyword that always represents the end of a block of code.", CompletionItemKind::KEYWORD),
        ("while", "Allows running a block of code constantly whilst a given condition is true.", CompletionItemKind::KEYWORD),
        ("for", "Allows running a block of code a certain number of times using a starting integer and ending integer. \
        The declared integer can also be used within the block of code, and is updated every loop.", CompletionItemKind::KEYWORD),
        ("foreach", "Allows running a block of code with a given collection and variable(s) tied to the items in said collection. \
        Each item is used in the block of code invidually, and the block runs as many times as there are items in the collection.", CompletionItemKind::KEYWORD),
        ("match", "Evaluates an expression and matches it against cases. Once a case matches, the case block runs. \
        A match statement can also have a default block of code using 'else'.", CompletionItemKind::KEYWORD),
        ("case", "Starts a case by following up with a value to match against a match statements expression. \
        Once this case statement matches, the matching stops and runs the block of code. However, if there is no 'break' \
        at the end of a case, the match statements falls through to the next case automatically and runs it's code, and so on \
        until a break statement is hit, or the else case is finally evaluated.", CompletionItemKind::KEYWORD),
        ("do", "Starts a new block of code. This is identical to 'then', but better represents the statements it is used for.", CompletionItemKind::KEYWORD),
        ("true", "A keyword representing 'true'", CompletionItemKind::KEYWORD),
        ("false", "A keyword representing 'false'", CompletionItemKind::KEYWORD),
        ("possible", "A boolean type that can have an attached condition using 'when()'. When this posssible is evaluated by the condition, \
        it becomes either true or false, and acts like either values with expressions and code interactions. \
        A possible with a condition will constantly check it's condition when some kind of expression or statement is run,
        immediately switching from true and false depending on the conditions newly evaluated value. You can stop a \
        possible from being evaluated by it's condition(s) by using 'freeze()'. \
        Possibles have their own unique interaction with true and false when unevaluated.", CompletionItemKind::KEYWORD),
        ("break", "Breaks out of a loop or statement if supported. This does not break out of tasks.", CompletionItemKind::KEYWORD),
        ("skip", "Skips over one iteration of a loop, moving back to the beginning of the loop block \
        and updating any values based on the loop as though one loop block was completed.", CompletionItemKind::KEYWORD),
        ("nil", "The 'nil' value of Max. Any variable of any type can be assigned nil, but it can be replaced \
        by resort values instead if you wish.", CompletionItemKind::KEYWORD),
        ("as", "Keyword used to indicate static type specification.", CompletionItemKind::KEYWORD),
        ("to", "Keyword in for loops to indicate where the variable reaches.", CompletionItemKind::KEYWORD),
        ("of", "Keyword used to indicate static element type specification in collection types.", CompletionItemKind::KEYWORD),
        ("in", "Keyword used to tie a variable to the individual elements of a collection being iterated over.", CompletionItemKind::KEYWORD),
        ("with", "Keyword used to signify a tasks parameters. This is always followed by parenthesis", CompletionItemKind::KEYWORD),
        ("return", "Signifies the return from a task, ending it early. A return statement can also carry a value with, \
        supplying a value that assigned itself to a task call.", CompletionItemKind::KEYWORD),
        ("yield", "Specific to coroutines, this yields the coroutine by returning, with an optional value. \
        Yielding preserves the position the coroutine currently is, meaning once the coroutine is run again, \
        it comes picks back up where it left after the yield.", CompletionItemKind::KEYWORD),
        ("emit", "Emits an event task that follows it. Since events are partial, the payload an event recieves (alike parameters) \
        are in order of the definitions of the event partials.", CompletionItemKind::KEYWORD),
        ("members", "Keyword that signifies defining the members of an enum or sub-members of an enum member.", CompletionItemKind::KEYWORD),
        ("not", "Inverses a given boolean value or boolean resulting from an expression. With unresolved possibles, \
        not does not affect it at all.", CompletionItemKind::OPERATOR),
        ("and", "Represents the AND logic operator, requiring the left hand and right hand expressions to both \
        evaluate to true in order to become true.", CompletionItemKind::OPERATOR),
        ("or", "Represents the OR logic operator, where if either the left hand or right hand expressions resolve to true, \
        the or expression resolves to true entirely", CompletionItemKind::OPERATOR),
        ("step", "Operator keyword used in for loops to specify the incremental value the for loop variable is added by every loop. \
        A negative step means the variable will step backwards.", CompletionItemKind::OPERATOR),
        ("input", "The global input() task", CompletionItemKind::FUNCTION),
        ("when", "The global when() task", CompletionItemKind::FUNCTION),
        ("freeze", "The global freeze() task", CompletionItemKind::FUNCTION),
        ("error", "The global error() task", CompletionItemKind::FUNCTION)
    ])
});

pub fn complete(text: &str, pos: Position, registry: &NativeRegistry) -> Vec<CompletionItem> {
    let context: Context = context(line_prefix(text, pos));

    match context {
        Context::None => vec![],

        Context::Member(chain) => {
            let scope = scope::build(text, pos.line + 1); // LSP is 0-based, spans are 1-based, so we accommodate
            complete_member(&chain, &scope, registry)
        }

        // int, float, ... plus the file's enum names
        Context::TypeName => {
            let scope = scope::build(text, pos.line + 1);

            let items: Vec<CompletionItem> = scope.enums.keys().map(
                |name| create_completion_item(name, "", CompletionItemKind::ENUM)
            ).collect();

            let total: Vec<CompletionItem> = TYPE_NAME_ITEMS.iter().cloned().chain(items).collect();
            total
        },
        // function, coroutine, event, native...
        Context::TaskKind => TASK_TYPE_NAME_ITEMS.clone(),
        // keywords, types, global tasks, module names, symbols, yada yada...
        Context::General => GENERAL_NAME_ITEMS.clone(),
    }
}