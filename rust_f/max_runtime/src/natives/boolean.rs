// ... Boolean methods? WHAT? These can be surprisingly useful, I tell you!

use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use max_bytecode::MaxType::*;
use super::{NativeTask, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\



pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\

    
    registry.register_task(
        NativeTarget::Type("BooleanType"),
        NativeTask::new(
            "then",
            None,
            vec![ParamSpec::required(BooleanType), ParamSpec::required(TaskType(0))],
            self::boolean_then
        )
    );
    
    registry.register_task(
        NativeTarget::Type("BooleanType"),
        NativeTask::new(
            "else",
            None,
            vec![ParamSpec::required(BooleanType), ParamSpec::required(TaskType(0))],
            self::boolean_else
        )
    );
    
    registry.register_task(
        NativeTarget::Type("BooleanType"),
        NativeTask::new(
            "match",
            None,
            vec![ParamSpec::required(BooleanType), ParamSpec::required(TaskType(0)), ParamSpec::required(TaskType(0))],
            self::boolean_match
        )
    );
    
    registry.register_task(
        NativeTarget::Type("BooleanType"),
        NativeTask::new(
            "expect",
            None,
            vec![ParamSpec::required(BooleanType), ParamSpec::Optional(Some(StringType))],
            self::boolean_expect
        )
    );


    // ----- REGISTERING TASKS ----- \\
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/


pub fn boolean_then(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Boolean(bool), Value::Lambda(lambda)] = args.as_slice() else {
        panic!("Boolean task then expects a boolean reciever and a lambda task")
    };

    let mut result = Value::Nil;

    if *bool == true {
        // No args!
        result = vm.call_lambda(lambda, vec![]);
    }

    result
}

pub fn boolean_else(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Boolean(bool), Value::Lambda(lambda)] = args.as_slice() else {
        panic!("Boolean task else expects a boolean reciever and a lambda task")
    };

    let mut result = Value::Nil;

    if *bool == false {
        result = vm.call_lambda(lambda, vec![]);
    }

    result
}

pub fn boolean_match(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // This is just then/else together!
    let [Value::Boolean(bool), Value::Lambda(truth_lambda), Value::Lambda(false_lambda)] = args.as_slice() else {
        panic!("Boolean task match expects a boolean reciever and a lambda task for both true and false")
    };

    let result;

    // `result` is always assigned to!
    if *bool == true {
        result = vm.call_lambda(truth_lambda, vec![]);
    } else {
        result = vm.call_lambda(false_lambda, vec![]);
    }

    result
}

pub fn boolean_expect(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let bool = match args_iter.next() {
        Some(Value::Boolean(bool)) => bool,

        _ => panic!("Boolean task expect expects a boolean reciever and a string error message")
    };

    let message = match args_iter.next() {
        Some(Value::String(str)) => str,

        Some(_) => panic!("Boolean task expect recieved a non-string message"),

        // Our norm string
        _ => String::from("Error to be handled: Expected a true value, recieved a false value.")
    };


    if bool == false {
        panic!("{}", message)
    }

    Value::Nil
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/


/* ----- HELPER FUNCTIONS ----- */