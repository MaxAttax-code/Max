use crate::{natives::{NativeRegistry, ParamSpec}, vm::{MaxType::*, Value, VirtualMachine}};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {
    let name = "iterables";


    // ----- REGISTERING TASKS ----- \\


    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "countChars",
            Some(DictionaryType(Some(Box::new(StringType)), Some(Box::new(IntegerType)))),
            vec![ParamSpec::required(StringType)],
            self::iterables_count_chars
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "countExistingChars",
            Some(DictionaryType(Some(Box::new(StringType)), Some(Box::new(IntegerType)))),
            vec![ParamSpec::required(StringType)],
            self::iterables_count_chars_existing
        )
    );

    
    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "alphabet",
            Some(ListType(Some(Box::new(StringType)))),
            StringType,
            self::iterables_alphabet
        )
    );
}

/**
    This function counts how many of the visible characters are inside a string.
    This counts even characters that don't appear.
    This returns the value as a Dictionary of Character: Amount
*/
pub fn iterables_count_chars(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    if args.len() != 1 {
        panic!("iterables.countChars intakes one argument");
    }

    let string = match args[0].clone() {
        Value::String(s) => s,

        _ => panic!("iterables.countChars expects a string argument")
    };

    // We have to get the keys and values as individual vectors
    let mut chars: Vec<Value> = vec![];
    let mut counts: Vec<Value> = vec![];

    for char in 'a'..='z' {
        let count /* inside the string! */ = count(char, &string);
        chars.push(Value::String(char.to_string()));
        counts.push(Value::Integer(count as i64));
    }

    // Create dictionary, even with type specifications!
    Value::new_dictionary(Some(String::from("StringType")), Some(String::from("IntegerType")), Some(false), chars, counts)
}

// Same as other function but no dups
pub fn iterables_count_chars_existing(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    if args.len() != 1 {
        panic!("iterables.countExistingChars intakes one argument");
    }

    let string = match args[0].clone() {
        Value::String(s) => s,

        _ => panic!("iterables.countExistingChars expects a string argument")
    };

    let mut chars: Vec<Value> = vec![];
    let mut counts: Vec<Value> = vec![];

    for char in 'a'..='z' {
        let count = count(char, &string);
        if count > 0 { // This is quite literally the only difference...
            chars.push(Value::String(char.to_string()));
            counts.push(Value::Integer(count as i64));
        }
    }

    Value::new_dictionary(Some(String::from("StringType")), Some(String::from("IntegerType")), Some(false), chars, counts)
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/


pub fn iterables_alphabet(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    let mut alphabet: Vec<Value> = vec![];

    // Loop through and add!
    for char in 'a'..='z' {
        let val = Value::String(char.to_string());
        alphabet.push(val);
    }

    Value::new_list(Some(String::from("StringType")), alphabet)
}


// |||||||||||||||||||||||||||||||||||||||| \\


// ----- HELPER FUNCTIONS ----- \\

/// Helper function to count how many of a character appears within a string
fn count(character_find: char, str: &String) -> usize {
    let mut count: usize = 0; // A count of how many letters exist

    for char in str.chars() {
        if char == character_find {
            count += 1;
        }
    }

    count
}