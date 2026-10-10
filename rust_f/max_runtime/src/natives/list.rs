use crate::{natives::{NativeProperty, NativeRegistry, ParamSpec}, vm::{MaxType::{self, *}, Value, VirtualMachine}};
use super::{NativeTask, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\


pub fn register_module(registry: &mut NativeRegistry) {

    // ----- REGISTERING TASKS ----- \\

    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "add",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required_none()],
            self::list_add
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "addRange",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::variadic_none()],
            self::list_add_range
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "concatenate",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(ListType(None))],
            self::list_concatenate
        )
    );
    
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "combine",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(ListType(None))],
            self::list_combine
        )
    );
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "insert",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(IntegerType), ParamSpec::required_none()],
            self::list_insert
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "removeAt",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(IntegerType)],
            self::list_remove_at
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "trimExcess",
            None,
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(IntegerType)],
            self::list_trim_excess
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "copy",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None))],
            self::list_copy
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "contains",
            Some(BooleanType),
            vec![ParamSpec::required(ListType(None)), ParamSpec::required_none()],
            self::list_contains
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "clear",
            None,
            vec![ParamSpec::required(ListType(None))],
            self::list_clear
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "reverse",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None))],
            self::list_reverse
        )
    );
    
    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "join",
            Some(StringType),
            vec![ParamSpec::required(ListType(None)), ParamSpec::optional(StringType)],
            self::list_join
        )
    );

    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "map",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(TaskType(0))], // Please note the arity within 'TaskType' does not matter!
            self::list_map
        )
    );

    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "filter",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None)), ParamSpec::required(TaskType(0))],
            self::list_filter
        )
    );

    registry.register_task(
        NativeTarget::Type("ListType"),
        NativeTask::new(
            "getEnumerator",
            Some(DictionaryType(Some(Box::new(IntegerType)), None)), // We do guarantee than the enumerator keys are integers!
            vec![ParamSpec::required(ListType(None))],
            self::list_get_enumerator
        )
    );


    // ----- REGISTERING TASKS ----- \\

    
    registry.register_property(
        NativeTarget::Type("ListType"),
        NativeProperty::new(
            "length",
            Some(IntegerType),
            ListType(None),
            self::list_length
        )
    );
    
    registry.register_property(
        NativeTarget::Type("ListType"),
        NativeProperty::new(
            "isEmpty",
            Some(BooleanType),
            ListType(None),
            self::list_is_empty
        )
    );
    
    registry.register_property(
        NativeTarget::Type("ListType"),
        NativeProperty::new(
            "first",
            None,
            ListType(None),
            self::list_first
        )
    );
    
    registry.register_property(
        NativeTarget::Type("ListType"),
        NativeProperty::new(
            "last",
            None,
            ListType(None),
            self::list_last
        )
    );
}


/*
    --------------------------
    ----- TASK FUNCTIONS -----
    --------------------------
*/


pub fn list_add(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), item] = args.as_slice() else {
        panic!("List task add expects a list receiver and one item");
    };

    let mut list = list.borrow_mut();

    if let Some(element_type) = &list.element_type {
        let expected = MaxType::string_name_to_max(element_type);

        MaxType::ensure_compatibility(&expected, item);
    }

    // Simply push to the list, as it is a Mutable reference, so it automatically changes for us without having to reassign anything!
    list.items.push(item.clone());
    drop(list);
    // We notify the VM that the collection has changed (for possible values to react to)
    vm.notify_collection_changed(&args[0]);

    Value::Nil
}

pub fn list_add_range(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list_handle = match args_iter.next() {
        Some(Value::List(l)) => l,

        _ => panic!("List task addRange requires a list reciever")
    };

    let mut list = list_handle.borrow_mut();

    // Just push in the remaining elements!
    for element in args_iter {
        // Yo ho ho! It's a repeating flow!
        if let Some(element_type) = &list.element_type {
            let expected = MaxType::string_name_to_max(element_type);

            MaxType::ensure_compatibility(&expected, &element);
        }

        list.items.push(element);
    }

    drop(list);
    vm.notify_collection_changed(&Value::List(list_handle));
    Value::Nil
}

pub fn list_concatenate(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::List(appendable)] = args.as_slice() else {
        panic!("List task add (multiple variant) expects a list receiver and items of any amount");
    };

    let mut list = list.borrow_mut();
    let appendable = appendable.borrow();

    if let Some(element_type) = &list.element_type {
        let expected = MaxType::string_name_to_max(element_type);

        for item in &appendable.items {
            MaxType::ensure_compatibility(&expected, item);
        }
    }

    let items = appendable.items.clone();

    list.items.extend(items);
    drop(appendable);
    drop(list);

    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn list_combine(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::List(appendable)] = args.as_slice() else {
        panic!("List task add (multiple variant) expects a list receiver and items of any amount");
    };

    let mut list = list.borrow_mut();
    let appendable = appendable.borrow();

    if let Some(element_type) = &list.element_type {
        let expected = MaxType::string_name_to_max(element_type);

        for item in &appendable.items {
            MaxType::ensure_compatibility(&expected, item);
        }
    }

    let items = appendable.items.clone();

    // Difference from concatenate is that it does not allow duplicates!
    for item in items {
        if list.items.contains(&item) {
            continue;
        }
        else {
            list.items.push(item);
        }
    }

    drop(appendable);
    drop(list);

    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn list_insert(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::Integer(index), item] = args.as_slice() else {
        panic!("List task insert expects a list reciever, index and one item")
    };

    let mut list = list.borrow_mut();

    if let Some(element_type) = &list.element_type {
        let expected = MaxType::string_name_to_max(element_type);

        MaxType::ensure_compatibility(&expected, item);
    }

    list.items.insert(*index as usize, item.clone());
    drop(list);

    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn list_remove_at(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::Integer(index)] = args.as_slice() else {
        panic!("List task removeAt expects a list reciever and an index")
    };

    let mut list = list.borrow_mut();

    list.items.remove(*index as usize);
    drop(list);

    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn list_trim_excess(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::Integer(count)] = args.as_slice() else {
        panic!("List task trimExcess expects a list reciever and a count")
    };

    let mut list = list.borrow_mut();

    list.items.truncate(*count as usize);
    drop(list);

    vm.notify_collection_changed(&args[0]);
    Value::Nil
}

pub fn list_copy(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list)] = args.as_slice() else {
        panic!("List task copy expects a list reciever only")
    };

    let list = list.borrow();
    // Here we DO want to clone the items raw!
    let copied_items = list.items.clone();

    // No element type is given, that's for the user to decide!
    Value::new_list(None, copied_items)
}

pub fn list_contains(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), item] = args.as_slice() else {
        panic!("List task contains expects a list reciever and one item")
    };

    let list = list.borrow();
    let contains_answer: bool = list.items.contains(item);

    Value::Boolean(contains_answer)
}

pub fn list_clear(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list)] = args.as_slice() else {
        panic!("List task clear expects a list reciever only")
    };

    let mut list = list.borrow_mut();

    list.items.clear();
    drop(list);

    vm.notify_collection_changed(&args[0]);

    Value::Nil
}

pub fn list_reverse(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list)] = args.as_slice() else {
        panic!("List task reverse expects a list reciever only")
    };

    let mut list = list.borrow_mut();

    list.items.reverse();
    drop(list);

    vm.notify_collection_changed(&args[0]);

    Value::Nil
}

pub fn list_join(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list = match args_iter.next() {
        Some(Value::List(l)) => l,

        _ => panic!("List task join expected a list reciever"),
    };

    // We can handle if the seperator is not supplied!
    let separator = match args_iter.next() {
        Some(Value::String(s)) => s,

        Some(_) => panic!("Global task input expected string prompt"),

        None => String::new(), // Just ""
    };

    let list = list.borrow();

    // We perform on each item in the list our helper function
    let joined_str = list
        .items
        .iter()
        .map(|item| value_to_string_via_seperator(item, &separator))
        .collect::<Vec<String>>()
        .join(&separator);

    Value::String(joined_str)
}

pub fn list_map(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::Lambda(lambda)] = args.as_slice() else {
        panic!("List task map expects a list reciever and a lambda predicate")
    };

    let (list_items, list_type) = {
        let list = list.borrow();
        (list.items.clone(), list.element_type.clone())
    };

    // Via the VMs built in method, call a lambda for each value in the list!
    let mapped_items = list_items
        .into_iter()
        .map(|item| vm.call_lambda(lambda, vec![item]))
        .collect();

    // Create a new list via the mapped items
    // The element type is resupported if needed
    Value::new_list(list_type, mapped_items)
}

pub fn list_filter(vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::List(list), Value::Lambda(lambda)] = args.as_slice() else {
        panic!("List task map expects a list reciever and a lambda predicate")
    };

    let (list_items, list_type) = {
        let list = list.borrow();
        (list.items.clone(), list.element_type.clone())
    };

    // I.e. A Vector of items that PASSED the predicate
    let mut filtered_items: Vec<Value> = Vec::new();

    // Perform the lambda on each item
    for item in list_items {
        // Execute the lambda with the current item as its argument
        let condition = vm.call_lambda(&lambda, vec![item.clone()]);

        // Validate that the predicate returned a Boolean
        match condition {
            // If true, keep it, else filter it out!
            Value::Boolean(keep) => {
                if keep {
                    filtered_items.push(item);
                }
            }
            _ => panic!("Filter predicate must return a Boolean value"),
        }
    }

    // Now return the new filtered list!
    Value::new_list(list_type, filtered_items)
}

pub fn list_get_enumerator(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let list = match &args[0] {
        Value::List(list) => list.borrow(),

        _ => panic!("List task getEnumerator expected a list reciever")
    };
    
    // Very nicely just iterate throguh the items, enumerate it, ->
    // map each index and item, giving us their Values (items are unfortunately cloned), ->
    // Then we unzip and are left with the vectors!
    let (indexes, items): (Vec<Value>, Vec<Value>) = list
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| (Value::Integer(index as i64), item.clone()))
        .unzip();

    // Now into one dictionary! With the types very nicely put for us too :)))))
    // We always allow true, because a list can have duplicates whenever it wants
    let new_dict = Value::new_dictionary(Some(String::from("IntegerType")), list.element_type.clone(), Some(true), indexes, items);
    new_dict
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/


pub fn list_length(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let list = match s_arg {
        Value::List(list) => list,

        _ => panic!("List property length was called on {}", s_arg)
    };

    let list = list.borrow();
    let length = list.items.len();

    Value::Integer(length as i64)
}

pub fn list_is_empty(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let list = match s_arg {
        Value::List(list) => list,

        _ => panic!("List property isEmpty was called on {}", s_arg)
    };

    let list = list.borrow();
    let length = list.items.len();

    // Just check if the length is 0
    Value::Boolean(length == 0)
}

pub fn list_first(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let list = match s_arg {
        Value::List(list) => list,

        _ => panic!("List property length was called on {}", s_arg)
    };

    let list = list.borrow();
    let first = list.items.first().unwrap_or(&Value::Nil);

    // First can be any kind of value, keep in mind
    first.clone()
}

pub fn list_last(_vm: &mut VirtualMachine, s_arg: Value) -> Value {
    let list = match s_arg {
        Value::List(list) => list,

        _ => panic!("List property length was called on {}", s_arg)
    };

    let list = list.borrow();
    let last = list.items.last().unwrap_or(&Value::Nil);

    last.clone()
}


/* ----- HELPER FUNCTIONS ----- */

fn value_to_string_via_seperator(value: &Value, seperator: &str) -> String {
    let string = match value {
        // All of em into a string!
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(bl) => bl.to_string(),
        Value::Possible(_) => value.to_string(),
        Value::Nil => String::from("nil"),
        Value::List(list) => {
            let items = list.borrow().items.clone();
            let collected: Vec<String> = items.iter().map(|item| value_to_string_via_seperator(item, seperator)).collect();

            // All the items will be joined with the seperator inside as well!
            collected.join(seperator)
        }
        Value::Dictionary(dictionary) => {
            let dict = dictionary.borrow();

            let mapped: Vec<String> = dict.keys.iter()
                            .zip(dict.values.iter())
                            .map(|(k, v)| format!("{}{}{}", value_to_string_via_seperator(k, seperator), seperator, value_to_string_via_seperator(v, seperator)))
                            .collect();

            mapped.join(seperator)
        }
        // Unfortunately, coroutines don't pass in their name. So this is the best we can do
        Value::Coroutine(_) => String::from("CoroutineInstance"),
        Value::Lambda(lam) => {
            let lambda = &lam.definition;

            lambda.id.to_string()
        }
    };

    string
}