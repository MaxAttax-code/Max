use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use max_bytecode::MaxType::*;
use super::{NativeTask, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\


    registry.register_task(
        NativeTarget::Global,
        NativeTask::new(
            "input",
            Some(StringType),
            vec![ParamSpec::optional(StringType)],
            self::global_input
        )
    );

    registry.register_task(
        NativeTarget::Global,
        NativeTask::new(
            "when",
            None,
            vec![ParamSpec::required(PossibleType), ParamSpec::required(TaskType(0))],
            self::global_when
        )
    );

    registry.register_task(
        NativeTarget::Global,
        NativeTask::new(
            "freeze",
            None,
            vec![ParamSpec::required(PossibleType)],
            self::global_freeze
        )
    );

    registry.register_task(
        NativeTarget::Global,
        NativeTask::new(
            "error",
            None,
            vec![ParamSpec::optional(StringType)],
            self::global_error
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/


/// An input function for allowing input, whether with a string argument or not
pub fn global_input(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // We can handle optional prompt argument when it appears
    let mut args_iter = args.into_iter();

    // Extract optional prompt parameter if supplied
    let prompt: Option<String> = match args_iter.next() {
        Some(Value::String(s)) => Some(s),

        Some(_) => panic!("Global task input expected string prompt"),

        None => None, // Parameter was omitted by the caller
    };

    // We can display it if present
    if let Some(p) = prompt {
        print!("{}", p);

        // Huh, we just put a use statement right in here?
        use std::io::Write;

        let _ = std::io::stdout().flush();
    }

    // Now we can read an input line from standard input
    let mut input_buffer = String::new();
    std::io::stdin()
        .read_line(&mut input_buffer)
        .expect("Failed to read line from stdin");

    // Trim whitespaces. Too bad if you wanted them
    let trimmed = input_buffer.trim_end_matches(['\r', '\n']).to_string();

    Value::String(trimmed)
}

pub fn global_when(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Possible(possible), Value::Lambda(condition)] = args.as_slice() else {
        panic!("when expects a Possible value and a zero-argument condition lambda");
    };

    // Sorry, no params!
    if condition.definition.arity != 0 {
        panic!("Global task when condition lambda must take no arguments");
    }

    vm.attach_possible_condition(possible, condition);
    // We send Nil, so remember that when() automatically updates the value!
    Value::Nil
}

pub fn global_freeze(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let possible = match &args[0] {
        Value::Possible(possible) => possible,

        _ => panic!("Global task freeze expected a possible that had been attached a condition")
    };

    // Alright, now we call freeze!
    vm.freeze_possible(possible);
    Value::Nil
}

pub fn global_error(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    // Message is optional
    let msg = match args_iter.next() {
        Some(Value::String(str)) => str,
        Some(_) => String::from("Invalid error msg given"),

        None => String::from("Error occurred in code")
    };

    eprintln!("{msg}");

    std::process::exit(1);
}