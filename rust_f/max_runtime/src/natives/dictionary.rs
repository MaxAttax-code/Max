use crate::{natives::{NativeRegistry, ParamSpec}, vm::{MaxType::*, Value, VirtualMachine}};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "containsKey",
            Some(BooleanType),
            vec![ParamSpec::required(DictionaryType(None, None)), ParamSpec::required_none()],
            self::dictionary_contains_key
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "add",
            None,
            vec![ParamSpec::required(DictionaryType(None, None)), ParamSpec::required_none(), ParamSpec::required_none()],
            self::dictionary_add
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "remove",
            None,
            vec![ParamSpec::required(DictionaryType(None, None)), ParamSpec::required_none()],
            self::dictionary_remove
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "copy",
            Some(DictionaryType(None, None)),
            vec![ParamSpec::required(DictionaryType(None, None))],
            self::dictionary_copy
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "softCopy",
            Some(DictionaryType(None, None)),
            vec![ParamSpec::required(DictionaryType(None, None))],
            self::dictionary_soft_copy
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "clear",
            None,
            vec![ParamSpec::required(DictionaryType(None, None))],
            self::dictionary_clear
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "allowDuplicates",
            None,
            vec![ParamSpec::required(BooleanType)],
            self::dictionary_allow_duplicates
        )
    );

    registry.register_task(
        NativeTarget::Type("DictionaryType"),
        NativeTask::new(
            "attemptGet",
            None,
            vec![ParamSpec::required_none()],
            self::dictionary_attempt_get
        )
    );


    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Type("DictionaryType"),
        NativeProperty::new(
            "length",
            Some(IntegerType),
            DictionaryType(None, None),
            self::dictionary_length
        )
    );

    registry.register_property(
        NativeTarget::Type("DictionaryType"),
        NativeProperty::new(
            "isEmpty",
            Some(BooleanType),
            DictionaryType(None, None),
            self::dictionary_is_empty
        )
    );

    registry.register_property(
        NativeTarget::Type("DictionaryType"),
        NativeProperty::new(
            "keys",
            Some(ListType(None)),
            DictionaryType(None, None),
            self::dictionary_keys
        )
    );

    registry.register_property(
        NativeTarget::Type("DictionaryType"),
        NativeProperty::new(
            "values",
            Some(ListType(None)),
            DictionaryType(None, None),
            self::dictionary_values
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/

// Keep in mind that since these are called upon a variable of a type, they always intake a self param \\


pub fn dictionary_contains_key(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Dictionary(dict), key] = args.as_slice() else {
        panic!("Dictionary task containsKey expects a dictionary reciever and a key")
    };

    let dict = dict.borrow();

    let contains = dict.keys.contains(&key);

    Value::Boolean(contains)
}

pub fn dictionary_add(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Dictionary(dict), key, value] = args.as_slice() else {
        panic!("Dictionary task add expects a dictionary reciever, a key and a value")
    };

    let mut dict = dict.borrow_mut();

    // Determine if a valid key type is being given
    if !DictionaryType(None, None).valid_dictkey_value(key) {
        panic!("Invalid key type given to add to dictionary: {}", key.get_runtime_typename().unwrap_or(String::from("NoType")))
    }

    // Duplicates check!
    if !dict.allow_duplicates.unwrap_or(false) {
        // Go through each entry (keys.len() = values.len())
        for index in 0..dict.keys.len() {
            let keyt = &dict.keys[index];

            if keyt.eq(key) {
                panic!("Attempted to add duplicate entry to dictionary of key '{}'", keyt)
            }
        }
    }

    // They are pushed at the same time
    dict.keys.push(key.clone());
    dict.values.push(value.clone());
    drop(dict);
    // Just like lists, we tell the VM this dictionary has been changed
    vm.notify_collection_changed(&args[0]);

    Value::Nil
}

pub fn dictionary_remove(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Dictionary(dict), key] = args.as_slice() else {
        panic!("Dictionary task add expects a dictionary reciever, a key and a value")
    };

    let mut dict = dict.borrow_mut();

    // Duplicates check!
    if let Some(allow_dups) = dict.allow_duplicates && !allow_dups {
        for index in 0..dict.keys.len() {
            let keyt = &dict.keys[index];

            if keyt.eq(key) {
                dict.keys.remove(index);
                dict.values.remove(index);
            }
        }
    }

    drop(dict);
    
    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn dictionary_copy(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let dict = match &args[0] {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary task copy expected a dictionary reciever")
    };

    // Clone the shared handle rather than allocating and cloning every entry.
    // Since we are cloning the Rc Ref, this is a memory-efficient copy
    // Keep in mind that due to this, we are also copying allowedDuplicates, keyType and valueType!
    Value::Dictionary(dict.clone())
}

pub fn dictionary_soft_copy(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let dict = match &args[0] {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary task softCopy expected a dictionary reciever")
    };

    // Does not copy the key and value types
    let copied = dict.clone();
    let mut borrowed = copied.borrow_mut();
    borrowed.key_type = None; borrowed.value_type = None;

    // We drop to allow us to return the copied value
    drop(borrowed);

    Value::Dictionary(copied)
}

pub fn dictionary_clear(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let dict = match &args[0] {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary task clear expected a dictionary reciever")
    };
    
    let mut dict = dict.borrow_mut();
    dict.keys.clear();
    dict.values.clear();
    drop(dict);

    vm.notify_collection_changed(&args[0]);
    
    Value::Nil
}

pub fn dictionary_allow_duplicates(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Dictionary(dict), Value::Boolean(bool)] = args.as_slice() else {
        panic!("Dictionary task allowDuplicates expects a dictionary reciever and a boolean other than possible") // Sorry, no possible option here!
    };
    
    let mut dict = dict.borrow_mut();
    dict.allow_duplicates = Some(*bool);
    drop(dict);
    
    vm.notify_collection_changed(&args[0]);
    
    Value::Nil
}

pub fn dictionary_attempt_get(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Dictionary(dict), key] = args.as_slice() else {
        panic!("Dictionary task attempGet expects a dictionary reciever and a key")
    };
    
    let dict = dict.borrow();

    for index in 0..dict.keys.len() {
        if &dict.keys[index] == key {
            let found = &dict.keys[index];
            return found.clone(); // Clone to return an individual copy of the ref
        }
    }
    
    // Nil fallback if not found
    Value::Nil
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ---------------------------
*/


pub fn dictionary_length(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let dict = match s_arg {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary property length expected a dictionary reciever")
    };
    
    let dict = dict.borrow();
    let length = dict.keys.len(); // There will be an equal amount of keys and values!
    
    Value::Integer(length as i64)
}

pub fn dictionary_is_empty(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let dict = match s_arg {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary property isEmpty expected a dictionary reciever")
    };
    
    let dict = dict.borrow();
    let is_empty = dict.keys.is_empty(); // Same logic here as length
    
    Value::Boolean(is_empty)
}

pub fn dictionary_keys(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let dict = match s_arg {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary property keys expected a dictionary reciever")
    };
    
    let dict = dict.borrow();
    // Iterating before cloning allows Rust to allocate a vector of the exact size before populating the vector!
    let keys: Vec<Value> = dict.keys.iter().cloned().collect();
    
    Value::new_list(dict.key_type.clone(), keys)
}

pub fn dictionary_values(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let dict = match s_arg {
        Value::Dictionary(dict) => dict,

        _ => panic!("Dictionary property keys expected a dictionary reciever")
    };
    
    let dict = dict.borrow();
    // Iterating before cloning allows Rust to allocate a vector of the exact size before populating the vector!
    let values: Vec<Value> = dict.values.iter().cloned().collect();
    
    Value::new_list(dict.value_type.clone(), values)
}


/* ----- HELPER FUNCTIONS ----- */

