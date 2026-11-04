use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use chrono::DateTime;
use max_bytecode::MaxType::{*};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {
    let name = "datetime";


    // ----- REGISTERING TASKS ----- \\


    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "isLeapYear",
            Some(BooleanType),
            vec![ParamSpec::required(IntegerType)],
            self::datetime_is_leap_year
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "timeSince",
            Some(FloatType),
            vec![ParamSpec::required(FloatType)],
            self::datetime_time_since
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "formatDate",
            Some(StringType),
            vec![ParamSpec::required(FloatType)],
            self::datetime_format_date
        )
    );

    
    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "currentUTC",
            Some(FloatType),
            FloatType,
            self::datetime_current_utc
        )
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "currentStringUTC",
            Some(StringType),
            StringType,
            self::datetime_string_current_utc
        )
    );
}


pub fn datetime_is_leap_year(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let Value::Integer(year) = &args[0] else {
        panic!("datetime.isLeapYear expects an integer year")
    };

    /*
        RECAP:
        A year is a leap year if:
         - It is divisible by 4
         - It is NOT divisible by 100 UNLESS ->
         - It is divisible by 400
    */
    
    // Pretty compact, but it works
    Value::Boolean((year % 4 == 0 && year % 100 != 0) || (year % 400 == 0))
}

pub fn datetime_time_since(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let seconds = match &args[0] {
        Value::Float(f) => *f,
        Value::Integer(i) => *i as f64,

        _ => panic!("datetime.timeSince expects a float seconds count")
    };

    if seconds < 0.0 || !seconds.is_finite() {
        panic!("datetime.timeSince expects a non-negative, finite seconds count");
    }

    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs_f64(seconds);

    // now - when. This only errs if `when` is in the future
    match std::time::SystemTime::now().duration_since(when) {
        Ok(elapsed) => Value::Float(elapsed.as_secs_f64()),
        Err(future) => Value::Float(-future.duration().as_secs_f64()),
    }
}

pub fn datetime_format_date(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let Value::Float(epoch_time) = &args[0] else {
        panic!("datetime.formatTime expects a float epoch time")
    };

    let stamped = DateTime::from_timestamp_secs(epoch_time.floor() as i64)
        .expect("Invalid timestamp given for datetime.formatDate");
    let formatted = stamped.format("%Y-%m-%d").to_string();

    Value::String(formatted)
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/


pub fn datetime_current_utc(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    let now = std::time::SystemTime::now();
    let since = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_else(
        |e| { eprintln!("Error occured whilst attempting to get the current time via datetime.currentUTC"); e.duration() }
    );
    let duration = since.as_secs_f64();

    Value::Float(duration)
}

pub fn datetime_string_current_utc(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    let now = std::time::SystemTime::now();

    let val: Value = match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => {
            let total_seconds = duration.as_secs();
            let hours = (total_seconds / 3600) % 24;
            let minutes = (total_seconds / 60) % 60;
            let seconds = total_seconds % 60;

            let str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

            Ok(Value::String(str))
        }

        Err(_) => Err("System time is before UNIX EPOCH!"),
    }.unwrap_or_else(
        |e| { eprintln!("Error occured whilst attempting to get string representation of the current time via datetime.currentStringUTC"); Value::String(e.into()) }
    );

    val
}


// |||||||||||||||||||||||||||||||||||||||| \\


// ----- HELPER FUNCTIONS ----- \\