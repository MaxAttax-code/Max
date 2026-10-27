use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use max_bytecode::MaxType::*;
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\

    // Yes we unfortunately must create duplicate funcs for integers and floats seperately
    registry.register_task(
        NativeTarget::Type("FloatType"),
        NativeTask::new(
            "abs",
            Some(FloatType),
            vec![ParamSpec::required(FloatType)],
            self::number_abs
        )
    );

    registry.register_task(
        NativeTarget::Type("IntegerType"),
        NativeTask::new(
            "abs",
            Some(IntegerType),
            vec![ParamSpec::required(IntegerType)],
            self::number_abs
        )
    );

    registry.register_task(
        NativeTarget::Type("FloatType"),
        NativeTask::new(
            "toString",
            Some(FloatType),
            vec![ParamSpec::required(FloatType)],
            self::number_to_string
        )
    );
    
    registry.register_task(
        NativeTarget::Type("IntegerType"),
        NativeTask::new(
            "toString",
            Some(IntegerType),
            vec![ParamSpec::required(IntegerType)],
            self::number_to_string
        )
    );
    
    registry.register_task(
        NativeTarget::Type("FloatType"),
        NativeTask::new(
            "clamp",
            Some(FloatType),
            vec![ParamSpec::required(FloatType), ParamSpec::required(FloatType), ParamSpec::required(FloatType)],
            self::number_clamp
        )
    );
    
    registry.register_task(
        NativeTarget::Type("IntegerType"),
        NativeTask::new(
            "clamp",
            Some(IntegerType),
            vec![ParamSpec::required(IntegerType), ParamSpec::required(IntegerType), ParamSpec::required(IntegerType)],
            self::number_clamp
        )
    );
    
    registry.register_task(
        NativeTarget::Type("FloatType"),
        NativeTask::new(
            "round",
            Some(FloatType),
            vec![ParamSpec::required(FloatType), ParamSpec::required(IntegerType)],
            self::float_round
        )
    );


    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Type("IntegerType"),
        NativeProperty::new(
            "isEven",
            Some(BooleanType),
            IntegerType,
            self::integer_is_even
        )
    );

    registry.register_property(
        NativeTarget::Type("IntegerType"),
        NativeProperty::new(
            "isOdd",
            Some(BooleanType),
            IntegerType,
            self::integer_is_odd
        )
    );

    registry.register_property(
        NativeTarget::Type("FloatType"),
        NativeProperty::new(
            "decimalPlaces",
            Some(IntegerType),
            FloatType,
            self::float_decimal_places
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/

// Keep in mind that since these are called upon a variable of a type, they always intake a self param \\


/// This function applies to both integer and float values. Pretty neat ay?
pub fn number_abs(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // We set the integer as a float for now
    let value = match args[0] {
        Value::Integer(i) => i as f64,
        Value::Float(f) => f,

        _ => panic!("Only call .abs() on number types!")
    };

    let final_val = value.abs();

    // If the original Value was an integer, set the final value back to an integer
    match args[0] {
        Value::Integer(_) => Value::Integer(final_val as i64),
        Value::Float(_) => Value::Float(final_val),

        _ => panic!("You have reached a panic statement that should never have been called. Please identify the error at fn 'number_abs'.")
    }
}

pub fn number_to_string(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let val = match args[0] {
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),

        _ => panic!("This method of to_string is only built for number types")
    };

    Value::String(val)
}

pub fn number_clamp(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // Check if any argument is a float to determine the calculation type
    // This works because... Well if it's an Integer they all must be integer values!
    let is_float = args.iter().any(|arg| matches!(arg, Value::Float(_)));

    // If for a Float
    if is_float {
        let number = value_to_f64(&args[0]).unwrap_or(0.0);
        let minimum = value_to_f64(&args[1]).unwrap_or(0.0);
        let maximum = value_to_f64(&args[2]).unwrap_or(0.0);

        if minimum > maximum {
            panic!("The minimum threshold for clamp() cannot be larger than the maximum threshold");
        }

        Value::Float(number.clamp(minimum, maximum))
    }
    // If for an Integer
    else {
        let number = value_to_i64(&args[0]).unwrap_or(0);
        let minimum = value_to_i64(&args[1]).unwrap_or(0);
        let maximum = value_to_i64(&args[2]).unwrap_or(0);

        if minimum > maximum {
            panic!("The minimum threshold for clamp() cannot be larger than the maximum threshold");
        }

        Value::Integer(number.clamp(minimum, maximum))
    }
}

pub fn float_round(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {

    let float_val = value_to_f64(&args[0]).unwrap_or(0_f64);
    let decimal_places = value_to_i64(&args[1]).unwrap_or(0_i64);

    if decimal_places > 15 {
        panic!("A decimal place value beyond 15 is too many for round()! It may cause floating-point inaccuracies");
    } else if decimal_places < 0 {
        panic!("A decimal place value cannot be negative for round(). It must be from 0 - 15.")
    }

    // We do 10^x, with x as decimal places
    let factor = 10_f64.powi(decimal_places as i32);
    /*
        Then we do basic math by rounding the multiplied float value, before dividing it by the factor again.
        This results in the float_val returning to it's original value with some decimal places cut off by round()
        E.g.
        (3.1415 * 10^2) / 10^2 -> 314.15 (round here) / 10^2 -> 314 / 100 -> 3.14!
    */
    let final_val = (float_val * factor).round() / factor;

    Value::Float(final_val)
}

pub fn integer_is_even(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let int_value = value_to_i64(&s_arg).unwrap_or(0);

    if int_value % 2 == 0 {
        return Value::Boolean(true);
    }

    Value::Boolean(false)
}

pub fn integer_is_odd(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let int_value = value_to_i64(&s_arg).unwrap_or(0);

    if int_value % 2 == 1 {
        return Value::Boolean(true);
    }

    Value::Boolean(false)
}

pub fn float_decimal_places(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let float_value = value_to_f64(&s_arg).unwrap_or(0_f64);

    let decimal_places = f64_decimal_places(&float_value);

    Value::Integer(decimal_places)
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ---------------------------
*/



/* ----- HELPER FUNCTIONS ----- */

/// Convert a Value, if an integer or float, into an f64
fn value_to_f64(val: &Value) -> Option<f64> {
    match val {
        Value::Float(f) => Some(*f),
        Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// Convert a Value, if an integer or float, into an f64
fn value_to_i64(val: &Value) -> Option<i64> {
    match val {
        Value::Float(f) => Some(*f as i64),
        Value::Integer(i) => Some(*i),
        _ => None,
    }
}

/// Get the number of decimal places within a float
fn f64_decimal_places(val: &f64) -> i64 {
    let float_as_string = val.to_string();

    // A float will have two parts, which are seperated by the '.'!
    let split = float_as_string.split('.');
    let parts: Vec<&str> = split.collect();
    // Now get the length of the fractional part (i.e. last index of the Vector)
    let length: usize = parts.last().iter().len();

    // If there IS a fractional part!
    // Because a float can be a value of e.g. 5, so no '.' exists there!
    if parts.len() as i64 > 1 {
        return length as i64;
    }

    0
}