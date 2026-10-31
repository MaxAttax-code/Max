use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use max_bytecode::MaxType::{self, *};
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

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "toDictionary",
            Some(DictionaryType(None, None)),
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(ListType(None))],
            self::iterables_to_dictionary
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "mode",
            Some(NoType),
            // NoType, since we allow both List types and Dict types
            vec![ParamSpec::required(NoType)],
            self::iterables_mode
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "middleValue",
            Some(NoType),
            vec![ParamSpec::required(ListType(None))],
            self::iterables_median
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "range",
            Some(FloatType),
            vec![ParamSpec::required(ListType(Some(Box::new(NumberType))))],
            self::iterables_range
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "inCommon",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(ListType(None)), ParamSpec::optional(BooleanType)],
            self::iterables_in_common
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "flatten",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None))],
            self::iterables_flatten
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "permutations",
            Some(IntegerType),
            vec![ParamSpec::required(ListType(None)), ParamSpec::optional(IntegerType)],
            self::iterables_permutations
        )
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "combinations",
            Some(IntegerType),
            vec![ParamSpec::required(ListType(None)), ParamSpec::optional(IntegerType)],
            self::iterables_combinations
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
    Value::new_dictionary(Some(MaxType::StringType), Some(MaxType::IntegerType), Some(false), chars, counts)
}

// Same as other function but no dups
pub fn iterables_count_chars_existing(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
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

    Value::new_dictionary(Some(MaxType::StringType), Some(MaxType::IntegerType), Some(false), chars, counts)
}

pub fn iterables_to_dictionary(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(keys_list), Value::List(values_list)] = args.as_slice() else {
        panic!("iterables.toDictionary expects a keys list and values list");
    };

    let keys_list = keys_list.borrow();
    let values_list = values_list.borrow();

    let new_dict = Value::new_dictionary(keys_list.element_type.clone(), values_list.element_type.clone(), Some(true), keys_list.items.clone(), values_list.items.clone());

    new_dict
}

pub fn iterables_mode(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    // Either list or dict
    let (lvalue, dvalue) = match args[0].clone() {
        Value::List(l) => (Some(l), None),
        Value::Dictionary(d) => (None, Some(d)),
        _ => (None, None)
    };

    if let Some(list) = lvalue {
        let val = list.clone();
        let list = val.borrow();
        let mut max_count = 0;
        // This means the chosen element out of a totally-equal array is always the first element
        let mut most_freq: &Value = &list.items[0];

        // Alike a hashmap, but we're just checking if the found amount is more than the previous
        for i in 0..list.items.len() {
            let mut count = 0;
            for j in 0..list.items.len() {
                if list.items[i] == list.items[j] {
                    count += 1;
                }
            }
            // More? New most frequent item found!
            if count > max_count {
                max_count = count;
                most_freq = &list.items[i];
            }
        }

        // And now return what we found the most freq!
        return most_freq.clone();
    }
    else if let Some(dict) = dvalue {
        let val = dict.clone();
        let dict = val.borrow();
        // For dictionaries, we find the most occuring value of course, not key!
        let mut max_count = 0;
        let mut most_freq: &Value = &dict.values[0];

        // Basically same logic since `values` is just a vec
        for i in 0..dict.values.len() {
            let mut count = 0;
            for j in 0..dict.values.len() {
                if dict.values[i] == dict.values[j] {
                    count += 1;
                }
            }
            if count > max_count {
                max_count = count;
                most_freq = &dict.values[i];
            }
        }

        return most_freq.clone();
    } else {
        panic!("iterables.mode expects a list or a dictionary");
    };
}

/// This is called median, but it is used for the native method `middleValue`
pub fn iterables_median(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let list = match args[0].clone() {
        Value::List(l) => l,

        _ => panic!("iterables.median expects a list")
    };
    let list = list.borrow();

    // Find how many elements there are...
    let count = list.items.len();

    // What'd be the middle count
    // We round up for if the items are an even amount
    let middle = ((count / 2) as f32).ceil();

    // And get the middle value
    let median = &list.items[middle as usize];

    return median.clone();
}

pub fn iterables_range(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let list = match args[0].clone() {
        Value::List(l) => l,

        _ => panic!("iterables.range expects a list")
    };
    let list = list.borrow();
    
    let mut smallest: f64 = 0_f64;
    let mut largest: f64 = 0_f64;

    for val in list.items.iter() {
        match val {
            Value::Integer(i) => {
                let f: f64 = *i as f64;

                if smallest > f {
                    smallest = f;
                }
                else if largest < f {
                    largest = f
                }
            }
            Value::Float(f) => {
                let f = *f;

                if smallest > f {
                    smallest = f;
                }
                else if largest < f {
                    largest = f
                }
            }

            _ => unreachable!("This error should be impossible to reach")
        }
    };

    // And now done!
    Value::Float(largest - smallest)
}

pub fn iterables_in_common(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list1 = match args_iter.next() {
        Some(v) => match v {
            Value::List(l) => l,
            _ => panic!("iterables.inCommon expects a list for the first argument")
        }

        None => panic!("iterables.inCommon expects two required lists")
    };
    let list1 = list1.borrow();

    let list2 = match args_iter.next() {
        Some(v) => match v {
            Value::List(l) => l,
            _ => panic!("iterables.inCommon expects a list for the second argument")
        }

        None => panic!("iterables.inCommon expects two required lists")
    };
    let list2 = list2.borrow();

    let dups_allowed: Option<bool> = match args_iter.next() {
        Some(v) => match v {
            Value::Boolean(b) => Some(b),
            _ => panic!("The optional parameter in iterables.inCommon is a boolean")
        }

        None => None
    };
    
    // Only get the pairs that are equal to each other
    let found = list1.items.iter().filter(|v1: &&Value| list2.items.contains(*v1));
    // Finalise by getting the collected values and seperating each into a single value
    let mut finalised: Vec<Value> = found.map(|v| v.clone()).collect();

    if let Some(allowed) = dups_allowed && allowed == false {
        // Since Value does not derive 'Eq', we have to take the long route
        let mut already_seen: Vec<Value> = Vec::new();
        finalised.retain(|val|
            match already_seen.contains(val) {
                // Already found? Skadoosh!
                true => false,
                false => {
                    already_seen.push(val.clone());
                    true
                }
            }
        );
    }

    Value::new_list(None, finalised)
}

pub fn iterables_flatten(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let list = match args[0].clone() {
        Value::List(l) => l,

        _ => panic!("iterables.flatten expects a list")
    };
    let list = list.borrow();

    let flattened: Vec<Value> = flatten(list.items.clone());

    Value::new_list(None, flattened)
}

pub fn iterables_permutations(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list = match args_iter.next() {
        Some(v) => {
            match v {
                Value::List(l) => l,

                _ => panic!("iterables.permutations expects a list argument first")
            }
        }

        None => panic!("iterables.permutations expects a list")
    };
    let list = list.borrow();

    let count = match args_iter.next() {
        Some(v) => {
            match v {
                Value::Integer(i) => Some(i),

                _ => panic!("iterables.permutations expects an optional integer argument")
            }
        }

        None => None
    };

    // The 'r' in the permutations formula
    // Always defaults to 2
    let r = count.unwrap_or(2);
    let n = list.items.len() as i64;

    // Quick little fn to msg about an overflow
    let overflow = || -> ! {
        panic!("Number overflow occured whilst attempting factorial in iterables.permutations")
    };

    let top = factorial_checked(n).unwrap_or_else(|| overflow());
    let bottom = factorial_checked(n - r).unwrap_or_else(|| overflow());

    let formulated = top / bottom;
    
    Value::Integer(formulated)
}

pub fn iterables_combinations(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list = match args_iter.next() {
        Some(v) => {
            match v {
                Value::List(l) => l,

                _ => panic!("iterables.combinations expects a list argument first")
            }
        }

        None => panic!("iterables.combinations expects a list")
    };
    let list = list.borrow();

    // --- Very identical to permutations, but a different formula --- \\

    let count = match args_iter.next() {
        Some(v) => {
            match v {
                Value::Integer(i) => Some(i),

                _ => panic!("iterables.combinations expects an optional integer argument")
            }
        }

        None => None
    };
    let r = count.unwrap_or(2);
    let n = list.items.len() as i64;

    let overflow = || -> ! {
        panic!("Number overflow occured whilst attempting factorial in iterables.combinations")
    };

    let top = factorial_checked(n).unwrap_or_else(|| overflow());
    // Bottom is split into r! and (n - r)!
    let rfact = factorial_checked(r).unwrap_or_else(|| overflow());
    let diffact = factorial_checked(n - r).unwrap_or_else(|| overflow());
    // Combined
    let bottom = rfact * diffact;

    // Totaled!
    let formulated = top / bottom;
    
    Value::Integer(formulated)
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

    Value::new_list(Some(MaxType::StringType), alphabet)
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

/// This performs factorial with i64, as that is what `Value::Integer` intakes.
/// 
/// The formula is performed with overflow safety. If the result is an overflow, return none
fn factorial_checked(num: i64) -> Option<i64> {
    (1..=num).try_fold(1i64, |acc, f| acc.checked_mul(f))
}

fn flatten(list: Vec<Value>) -> Vec<Value> {
    let mut flattened: Vec<Value> = Vec::new();

    for item in list.iter() {
        match item {
            Value::List(l) => {
                let list = l.borrow().items.clone();
                // Flatten further, and extend our list
                let flattened_further = flatten(list);
                flattened.extend(flattened_further);
            }
            
            _ => flattened.push(item.clone()),
        }
    }

    flattened
}