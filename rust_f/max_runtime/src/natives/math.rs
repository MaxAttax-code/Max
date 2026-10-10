use crate::{natives::{NativeRegistry, ParamSpec}, vm::{MaxType::*, Value, VirtualMachine}};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {
    let name = "math";


    // ----- REGISTERING TASKS ----- \\

    registry.register_task(
        NativeTarget::Module(name),
        NativeTask::new(
            "sqrt",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_sqrt
        )
    );

    registry.register_task(
        NativeTarget::Module(name),
        NativeTask::new(
            "cbrt",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_cbrt
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "root",
            Some(FloatType),
            vec![ParamSpec::required(NumberType), ParamSpec::required(NumberType)],
            self::math_root_of
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "log10",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_log10
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "log",
            Some(FloatType),
            vec![ParamSpec::required(NumberType), ParamSpec::required(NumberType)],
            self::math_log
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "loge",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_loge
        )
    );

    // Holy trigonometric ratios...

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "cos",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_cos
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "sin",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_sin
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "tan",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_tan
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "sec",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_sec
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "cosec",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_cosec
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "cotan",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_cotan
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iCos",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_icos
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iSin",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_isin
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iTan",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_itan
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iSec",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_isec
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iCosec",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_icosec
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iCotan",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_icotan
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "cosh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_cosh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "sinh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_sinh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "tanh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_tanh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iCosh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_icosh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iSinh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_isinh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "iTanh",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_itanh
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "reciprocal",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_reciprocal
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "floor",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_floor
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "ceil",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_ceil
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "round",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_round
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "abs",
            Some(FloatType),
            vec![ParamSpec::required(NumberType)],
            self::math_absolute
        )
    );


    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "PI",
            Some(FloatType),
            NumberType,
            self::math_pi
        )
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "phi",
            Some(FloatType),
            NumberType,
            self::math_phi
        )
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "rt2",
            Some(FloatType),
            NumberType,
            self::math_root2
        )
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "eul",
            Some(FloatType),
            NumberType,
            self::math_eul
        )
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "c",
            Some(IntegerType),
            NumberType,
            self::math_speed_of_light
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/


pub fn math_sqrt(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // Get arg, and perform sqrt on it if it's valid
    match &args[0] {
        Value::Float(f) => {
            if *f < 0.0 {
                panic!("Cannot compute the square root of a negative float")
            }
            Value::Float(f.sqrt())
        }

        Value::Integer(i) => {
            if *i < 0 {
                panic!("Cannot compute the square root of a negative integer")
            }
            // Integers don't have sqrt(), so convert to float first
            Value::Float((*i as f64).sqrt())
        }

        other => panic!("math.sqrt can intakes a float or integer argument, instead got {}", other)
    }
}

pub fn math_cbrt(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // Get arg, and perform sqrt on it if it's valid
    match &args[0] {
        Value::Float(f) => {
            if *f < 0.0 {
                // Calc positive base first (Rust doesn't allow cbrt on negative numbers)
                let positive_base = (f.abs()).cbrt();
                return Value::Float(-positive_base); // Then we negate
            }
            Value::Float(f.cbrt())
        }

        Value::Integer(i) => {
            if *i < 0 {
                let positive_base = (i.abs() as f64).cbrt();
                return Value::Float(-positive_base);
            }
            // Integers don't have sqrt(), so convert to float first
            Value::Float((*i as f64).cbrt())
        }

        other => panic!("math.sqrt can intakes a float or integer argument, instead got {}", other)
    }
}

pub fn math_root_of(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // The first argument is the base. The second is the root exponent

    // Get the base
    let base = match value_to_f64(&args[0]) {
        Some(b) => b,
        None => panic!("math.root expected numeric base, got {:?}", args[0]),
    };

    // Get the exponent
    let degree = match value_to_f64(&args[1]) {
        Some(d) => d,
        None => panic!("math.root expected numeric degree, got {:?}", args[1]),
    };

    // Nuh uh!
    if degree == 0.0 {
        panic!("math.root domain error: root degree cannot be zero");
    }

    // Handle positive bases and zero directly
    if base >= 0.0 {
        return Value::Float(base.powf(1.0 / degree));
    }

    // Negatives is where things get interesting
    // This checks if it is integer-like
    let is_integer_degree = degree.fract() == 0.0;

    if is_integer_degree {
        let degree_int = degree as i64;

        // Get if the degree is odd or even
        if degree_int % 2 != 0 {
            // Odd root of negative number: calculate root of positive base and negate!
            // We do this because Rust does not support this in it's built in function
            let positive_root = (-base).powf(1.0 / degree);
            Value::Float(-positive_root)
        } else {
            // Even root of negative number are not real numbers
            panic!("math.root domain error: cannot take even root of a negative number");
        }
    } else {
        // Fractional root of negative numbers are not real either
        panic!("math.root domain error: cannot take non-integer root of a negative number");
    }
}

pub fn math_log10(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let base_value = args[0].clone();
    let base_as = value_to_f64(&base_value);


    if let Some(base) = base_as {
        if base < 0_f64 {
            panic!("Cannot perform log10 on a negative number")
        } else if base == 0_f64 {
            // We like to give unique panicking to express the problem more concisely
            panic!("Performing log10 on 0 gives undefined")
        }

        let res = base.log10();

        return Value::Float(res);
    } else {
        panic!("Please give a valid number value for math.log10");
    }
}

pub fn math_log(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let base_value = &args[0];
    let exponent_value = &args[1];

    let base = match value_to_f64(base_value) {
        Some(b) => b,
        None => panic!("math.log expected a numeric base, got {:?}", base_value),
    };

    let exponent = match value_to_f64(exponent_value) {
        Some(e) => e,
        None => panic!("math.log expected a numeric exponent, got {:?}", exponent_value),
    };

    // We can't have this, because of obvious reasons
    if base <= 0.0 || base == 1.0 {
        panic!("math.log base must be positive and not equal to 1 (got {})", base);
    }
    if exponent <= 0.0 {
        panic!("math.log exponent must be greater than 0 (got {})", exponent);
    }

    // Rusts 'log' functon makes it so the base is the argument. Weird, huh?
    let res = base.log(exponent);

    Value::Float(res)
}

pub fn math_loge(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let base_value = args[0].clone();
    let base_as = value_to_f64(&base_value);

    if let Some(base) = base_as {
        if base < 0_f64 {
            panic!("Cannot perform loge on a negative number")
        } else if base == 0_f64 {
            panic!("Performing loge on 0 gives undefined")
        }

        let res = base.ln();

        return Value::Float(res);
    } else {
        panic!("Please give a valid number value for math.loge");
    }
}


/*
    |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|
    |     TRIGONOMETRIC FUNCS     |
    |_____________________________|
*/


/// Please keep in mind that all trigonometric ratio functions return in radians or require radian values
pub fn math_cos(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        // This is computed as radians
        let res = angle.cos();

        return Value::Float(res);
    } else {
        panic!("math.cos requires a valid number value");
    }
}

pub fn math_sin(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.sin();

        return Value::Float(res);
    } else {
        panic!("math.sin requires a valid number value");
    }
}

pub fn math_tan(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.tan();

        return Value::Float(res);
    } else {
        panic!("math.tan requires a valid number value");
    }
}

pub fn math_sec(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.cos();

        // 90, 270, 450... All give 0 which we can NOT divide by!
        if res == 0.0 {
            panic!("Cannot perform math.sec when the cosine of the given angle results into 0");
        }

        // We instead just to 1 / num! How nice
        return Value::Float(1_f64 / res);
    } else {
        panic!("math.sec requires a valid number value");
    }
}

pub fn math_cosec(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.sin();
        if res == 0.0 {
            panic!("Cannot perform math.cosec when the sine of the given angle results into 0");
        }

        return Value::Float(1_f64 / res);
    } else {
        panic!("math.cosec requires a valid number value");
    }
}

pub fn math_cotan(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.tan();
        
        if res == 0.0 {
            panic!("Cannot perform math.cotan when the tan of the given angle results into 0");
        }

        return Value::Float(1_f64 / res);
    } else {
        panic!("math.cotan requires a valid number value");
    }
}

pub fn math_isec(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        if num < 1.0 || num > -1.0 {
            panic!("math.iSec requires a value that is not between 1.0 and -1.0")
        }
        // Yes, we just do a reciprocal then proceed to do inverse norm trig
        let res = (1.0 / num).acos();

        // We instead just to 1 / num! How nice
        return Value::Float(res);
    } else {
        panic!("math.iSec requires a valid number value");
    }
}

pub fn math_icosec(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        // Interestingly, not allows between 1 and -1
        if num < 1.0 || num > -1.0 {
            panic!("math.iCosec requires a value that is not between 1.0 and -1.0")
        }
        let res = (1.0 / num).asin();

        return Value::Float(res);
    } else {
        panic!("math.iCosec requires a valid number value");
    }
}

pub fn math_icotan(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        if num == 0.0 {
            // Inverse cotan of 0 = PI / 2
            return Value::Float(std::f64::consts::FRAC_PI_2);
        }
        let res = (1.0 / num).atan();

        return Value::Float(res);
    } else {
        panic!("math.iCotan requires a valid number value");
    }
}

pub fn math_cosh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.cosh();

        return Value::Float(res);
    } else {
        panic!("math.cosh requires a valid number value");
    }
}

pub fn math_sinh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.sinh();

        return Value::Float(res);
    } else {
        panic!("math.sinh requires a valid number value");
    }
}

pub fn math_tanh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let angle_value = args[0].clone();
    let angle_as = value_to_f64(&angle_value);

    if let Some(angle) = angle_as {
        let res = angle.tanh();

        return Value::Float(res);
    } else {
        panic!("math.tanh requires a valid number value");
    }
}

pub fn math_icos(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        // Let it be real!
        if num > 1_f64 || num < -1_f64 {
            panic!("math.iCos expected a number in range of 1 or -1, got: {}", num);
        }

        // Just the inverse equivalents of the norm trig ratios
        let res = num.acos();

        return Value::Float(res);
    } else {
        panic!("math.iCos requires a valid number value");
    }
}

pub fn math_isin(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        if num > 1_f64 || num < -1_f64 {
            panic!("math.iSin expected a number in range of 1 or -1, got: {}", num);
        }
        let res = num.asin();

        return Value::Float(res);
    } else {
        panic!("math.iSin requires a valid number value");
    }
}

pub fn math_itan(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        let res = num.atan();

        return Value::Float(res);
    } else {
        panic!("math.iTan requires a valid number value");
    }
}

pub fn math_icosh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        // Must be more than 1.0
        if num < 1.0 {
            panic!("math.iCosh requires a number value >= 1");
        }

        // Just the inverse equivalents of the hyperbolic trig ratios
        let res = num.acosh();

        return Value::Float(res);
    } else {
        panic!("math.iCosh requires a valid number value");
    }
}

pub fn math_isinh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        // Yoohoo! We don't like math errors
        if num < 1.0 {
            panic!("math.iSinh requires an argument >= 1.0, got {}", num);
        }
        let res = num.asinh();

        return Value::Float(res);
    } else {
        panic!("math.iSinh requires a valid number value");
    }
}

pub fn math_itanh(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num_value = args[0].clone();
    let num_as = value_to_f64(&num_value);

    if let Some(num) = num_as {
        // There are limits to this as well!
        if num <= -1.0 || num >= 1.0 {
            panic!("math.iTanh requires an argument strictly between -1.0 and 1.0, got {}", num);
        }
        let res = num.atanh();

        return Value::Float(res);
    } else {
        panic!("math.iTanh requires a valid number value");
    }
}


/*
    |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|
    |     FLOAT-RELATED FUNCTIONS     |
    |_________________________________|
*/

pub fn math_reciprocal(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num = match value_to_f64(&args[0].clone()) {
        Some(num_val) => num_val,
        None => panic!("Expected a valid number value for math.reciprocal")
    };

    // As simply as PI!
    let final_val = 1.0 / num;

    Value::Float(final_val)
}

pub fn math_floor(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num = match value_to_f64(&args[0].clone()) {
        Some(num_val) => num_val,
        None => panic!("Expected a valid number value for math.floor")
    };

    let final_val = num.floor();

    Value::Float(final_val)
}

pub fn math_ceil(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num = match value_to_f64(&args[0].clone()) {
        Some(num_val) => num_val,
        None => panic!("Expected a valid number value for math.ceil")
    };

    let final_val = num.ceil();

    Value::Float(final_val)
}

pub fn math_round(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num = match value_to_f64(&args[0].clone()) {
        Some(num_val) => num_val,
        None => panic!("Expected a valid number value for math.round")
    };

    let final_val = num.round();

    Value::Float(final_val)
}

pub fn math_absolute(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let num = match value_to_f64(&args[0].clone()) {
        Some(num_val) => num_val,
        None => panic!("Expected a valid number value for math.absolute")
    };

    let final_val = num.abs();

    Value::Float(final_val)
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/

pub fn math_pi(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    // The self argument here is useless
    Value::Float(std::f64::consts::PI)
}

/// Not to be confused with pi, this returns the golden ratio
pub fn math_phi(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    // The self argument here is useless
    Value::Float(std::f64::consts::GOLDEN_RATIO)
}

pub fn math_eul(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    // The self argument here is useless
    Value::Float(std::f64::consts::E)
}

pub fn math_root2(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    // The self argument here is useless
    Value::Float(std::f64::consts::SQRT_2)
}

pub fn math_speed_of_light(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    Value::Integer(299792458)
}


// |||||||||||||||||||||||||||||||||||||||| \\


// HELPER FUNCTIONS \\

/// Convert a Value, if an integer or float, into an f64
fn value_to_f64(val: &Value) -> Option<f64> {
    match val {
        Value::Float(f) => Some(*f),
        Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}