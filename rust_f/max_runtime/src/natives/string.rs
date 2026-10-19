use crate::{natives::{NativeRegistry, ParamSpec}, vm::{MaxType::*, Value, VirtualMachine}};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\


    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "toUpper",
            Some(StringType),
            vec![ParamSpec::required(StringType)],
            self::string_to_upper
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "toLower",
            Some(StringType),
            vec![ParamSpec::required(StringType)],
            self::string_to_lower
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "trim",
            Some(StringType),
            vec![ParamSpec::required(StringType)],
            self::string_trim
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "contains",
            Some(BooleanType),
            vec![ParamSpec::required(StringType), ParamSpec::required(StringType)],
            self::string_contains
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "split",
            Some(ListType(Some(Box::new(StringType)))),
            vec![ParamSpec::required(StringType), ParamSpec::required(StringType)],
            self::string_split
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "toCharacters",
            Some(ListType(Some(Box::new(StringType)))),
            vec![ParamSpec::required(StringType)],
            self::string_to_characters
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "replace",
            Some(StringType),
            vec![ParamSpec::required(StringType), ParamSpec::required(StringType), ParamSpec::required(StringType)],
            self::string_replace
        )
    );

    registry.register_task(
        NativeTarget::Type("StringType"),
        NativeTask::new(
            "format",
            Some(StringType),
            vec![ParamSpec::required(StringType), ParamSpec::variadic_none()],
            self::string_format
        )
    );


    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Type("StringType"),
        NativeProperty::new(
            "isEmpty",
            Some(BooleanType),
            StringType,
            self::string_is_empty
        )
    );

    registry.register_property(
        NativeTarget::Type("StringType"),
        NativeProperty::new(
            "length",
            Some(IntegerType),
            StringType,
            self::string_length
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/


pub fn string_to_upper(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let string = match &args[0] {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toUpper: {}", args[0])
    };
    let new_string = string.to_uppercase();

    Value::String(new_string)
}

pub fn string_to_lower(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let string = match &args[0] {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toUpper: {}", args[0])
    };
    let new_string = string.to_lowercase();

    Value::String(new_string)
}

pub fn string_trim(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let string = match &args[0] {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toUpper: {}", args[0])
    };
    let new_string = string.trim().to_string();

    Value::String(new_string)
}

pub fn string_contains(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::String(str), Value::String(sub)] = args.as_slice() else {
        panic!("String task contains expects a string reciever and a substring")
    };

    let contained = str.contains(sub);

    Value::Boolean(contained)
}

pub fn string_split(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::String(str), Value::String(delimiter)] = args.as_slice() else {
        panic!("String task contains expects a string reciever and a delimiter")
    };

    let split = str.split(delimiter);
    let mut split_list = vec![];

    for sub in split {
        split_list.push(Value::String(String::from(sub)));
    }

    Value::new_list(Some(String::from("StringType")), split_list)
}

pub fn string_to_characters(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let str = match &args[0] {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toCharacters")
    };

    let chars: Vec<char> = str.chars().collect();
    let mut char_list = vec![];

    for char in chars {
        char_list.push(Value::String(char.to_string()));
    }

    Value::new_list(Some(String::from("StringType")), char_list)
}

pub fn string_replace(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::String(str), Value::String(to_find), Value::String(replacememt)] = args.as_slice() else {
        panic!("String task replace expects a string reciever, a searchable string pattern, and a replacer");
    };

    // Since we are creating a new string, this task doesn't mutate the string in-place!
    let new_str = str.replace(to_find, replacememt);

    Value::String(new_str)
}

pub fn string_format(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    /*
        Gets string reciever
        Gets all the given arguments to format with into a list
        Replaces all '{}' with the corresponding argument in the list
        Returns the new string, not modified in place!
    */

    // Extract the string reciever
    let str: String = match args_iter.next() {
        Some(Value::String(s)) => s,

        _=> panic!("String task format expected string reciever"),
    };

    // Convert all supplied arguments into their string representations.
    let formatters: Vec<String> = args_iter
        .map(format_to_string)
        .collect();

    let mut result = String::new();
    let mut formatter_idx = 0;

    // Scan the original string from left to right. (So that we avoid any '||' within an argument that was formatted in)
    let mut chars = str.chars().peekable();

    while let Some(ch) = chars.next() {
        // Peek a boo!
        if ch == '|' && chars.peek() == Some(&'|') {
            // Consume the second '|'.
            chars.next();

            // Ensure an argument exists for this placeholder.
            if formatter_idx >= formatters.len() {
                panic!(
                    "String task 'format' has more placeholders than arguments"
                );
            }

            // Insert the next argument without scanning its contents.
            result.push_str(&formatters[formatter_idx]);
            formatter_idx += 1;
        } else {
            // Just push in the character
            // Basically Bob the Builder-ing this
            result.push(ch);
        }
    }

    // Ensure every supplied argument was used.
    if formatter_idx < formatters.len() {
        panic!(
            "String task 'format' received more arguments than placeholders"
        );
    }

    Value::String(result)
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ---------------------------
*/


pub fn string_is_empty(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let string = match s_arg {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toUpper: {}", s_arg)
    };
    // Use chars().count() as len() counts bytes, which is not the standard 'human' way of reading strings
    let check = string.is_empty();

    Value::Boolean(check)
}

pub fn string_length(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let string = match s_arg {
        Value::String(s) => s,

        _ => panic!("Invalid self argument for toUpper: {}", s_arg)
    };
    // Use chars().count() as len() counts bytes, which is not the standard 'human' way of reading strings
    let length = string.chars().count();

    Value::Integer(length as i64)
}


/* ----- HELPER FUNCTIONS ----- */

/**
 * This method is essentially a copy of the fmt display values for each Value Type.
 * See fmt::Display for Value in vm.rs
 */
pub fn format_to_string(val: Value) -> String {
    match val {
        Value::String(s) => format!("{}", s),

        Value::Integer(n) => format!("{}", n),
        Value::Float(fl) => format!("{}", fl),

        Value::List(list) => {
            // We also want to format the inside items too!
            let formatted_items: Vec<String> = list.borrow().items.iter().map(|item| item.to_string()).collect();
            format!("[{}]", formatted_items.join(", "))
        }

        Value::Dictionary(dictionary) => {
            let dictionary = dictionary.borrow();
            // Just an empty one
            if dictionary.keys.is_empty() {
                return format!("{{}}");
            }

            let entries: Vec<String> = dictionary.keys
                .iter()
                .zip(dictionary.values.iter())
                .map(|(k, v)| format!("    ({}, {})", k, v))
                .collect(); // This is the final Vector

            format!("{{\n{}\n}}", entries.join(",\n"))
        }

        Value::Boolean(b) => format!("{}", b),
        Value::Possible(possible) => match possible.borrow().current {
            Some(value) => format!("{}", value),
            None => format!("possible"),
        },

        Value::Coroutine(coroutine) => format!("{:#?}", coroutine.borrow()),
        Value::Lambda(lambda) => format!("lambda#{}", lambda.definition.id),

        Value::EnumDefinition(enums) => format!("{}: {:#?}", enums.enum_name.borrow(), enums.enum_members),
        Value::EnumMember(memb) => format!("{}.{}", memb.enum_name.borrow(), memb.member_name.borrow()),

        Value::Nil => format!("nil"),
    }
}