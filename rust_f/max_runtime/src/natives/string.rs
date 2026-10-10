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