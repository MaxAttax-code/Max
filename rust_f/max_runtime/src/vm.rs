use serde::{Deserialize, Deserializer};
use serde::de::{self, MapAccess, Visitor};

use crate::vm::MaxType::{DictionaryType};
use crate::{bytecode::*, vm::{Value::List}};
use crate::natives::NativeRegistry;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use std::{panic};
use std::{collections::{HashMap, HashSet}, fmt::{self}, vec};
use std::rc::Weak;

/*
    -------------------
    --- VALUE TYPES ---
    -------------------
*/

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    List(Rc<RefCell<ListValue>>),
    Dictionary(Rc<RefCell<DictionaryValue>>),
    Boolean(bool),
    Coroutine(CoroutineHandle), // Special: Coroutines are tasks and variable Values too!
    Lambda(RuntimeLambda), // Given a RuntimeLambda instance to work with
    EnumDefinition(EnumStructure), // Allow standalone and typing
    EnumMember(EnumMember), // An enum member alone
    Possible(PossibleHandle),
    Nil
}

/// A special structure that a Lambda value holds.
/// Contains the definition (the lambda metadata),
/// and captures of the local variables from surrounding scopes within the lambda
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeLambda {
    pub definition: Arc<LambdaOperand>,
    pub captures: Arc<HashMap<String, VariableEntry>>,
}

/// A special structure that allows a list value to be borrowed via an RC reference
#[derive(Debug, Clone, PartialEq)]
pub struct ListValue {
    pub element_type: Option<String>,
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryValue {
    pub key_type: Option<String>,
    pub value_type: Option<String>,
    pub allow_duplicates: Option<bool>,
    pub keys: Vec<Value>,
    pub values: Vec<Value>,
}

/// A special handle containing a possible signal for a possible value
pub type PossibleHandle = Rc<RefCell<PossibleSignal>>;

/// An enum representing what kind of binding has been attached to a possible signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DependencyKey {
    Binding(usize),
    List(usize),
    Dictionary(usize),
    Possible(usize),
}

/// A structure that holds the info for a possible signal, including:
/// - The current evaluated bool (None if not evaluated)
/// - The condition lambda that possible value is running on
/// - The dependencies that the possible signal is attached with
#[derive(Debug, Clone, PartialEq)]
pub struct PossibleSignal {
    pub current: Option<bool>,
    condition: Option<RuntimeLambda>,
    dependencies: HashSet<DependencyKey>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Depending on it's typing
            Value::String(s) => write!(f, "{}", s),

            Value::Integer(n) => write!(f, "{}", n),
            Value::Float(fl) => write!(f, "{}", fl),

            Value::List(list) => {
                // We also want to format the inside items too!
                let formatted_items: Vec<String> = list.borrow().items.iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", formatted_items.join(", "))
            }

            Value::Dictionary(dictionary) => {
                let dictionary = dictionary.borrow();
                // Just an empty one
                if dictionary.keys.is_empty() {
                    return write!(f, "{{}}");
                }

                // Map each key-value pair into a formatted string with indentation (nice formatting, ey?)
                let entries: Vec<String> = dictionary.keys
                    .iter()
                    .zip(dictionary.values.iter())
                    .map(|(k, v)| format!("    ({}, {})", k, v))
                    .collect(); // This is the final Vector

                // Enclose in curly brackets with newline-separated indented entries
                write!(f, "{{\n{}\n}}", entries.join(",\n"))
            }

            Value::Boolean(b) => write!(f, "{}", b),
            Value::Possible(possible) => match possible.borrow().current {
                // Write either the evaluated bool or possible
                Some(value) => write!(f, "{}", value),
                None => write!(f, "possible"),
            },

            Value::Coroutine(coroutine) => write!(f, "{:#?}", coroutine.borrow()),
            Value::Lambda(lambda) => write!(f, "lambda#{}", lambda.definition.id),

            Value::EnumDefinition(enums) => write!(f, "{}: {:#?}", enums.identifier, enums.enum_members),
            Value::EnumMember(memb) => write!(f, "{}", memb.full_name()),

            Value::Nil => write!(f, "nil"),
        }
    }
}

// What's very nice about implementing unto Value is that now ANY Value object can use this!
impl Value {

    /// Create a new possible value, including it's PossibleSignal handle
    pub fn new_possible() -> Self {
        Value::Possible(Rc::new(RefCell::new(PossibleSignal {
            current: None,
            condition: None,
            dependencies: HashSet::new(),
        })))
    }

    /// Checks if the value, if a possible, has it's evaluation as None
    fn is_pending_possible(&self) -> bool {
        matches!(self, Value::Possible(possible) if possible.borrow().current.is_none())
    }

    /// Checks if the value, if a possible, has it's current evaluation finished
    fn resolved_possible(&self) -> Option<bool> {
        match self {
            Value::Possible(possible) => possible.borrow().current,
            _ => None,
        }
    }

    /// Helper function to quickly create a list value containing a reference of the given arguments.
    pub fn new_list(element_type: Option<String>, items: Vec<Value>) -> Self {
        Value::List(Rc::new(RefCell::new(ListValue { element_type, items })))
    }

    /// Helper function to quickly create a dictionary value via all the arguments given.
    pub fn new_dictionary(
        key_type: Option<String>,
        value_type: Option<String>,
        allow_duplicates: Option<bool>,
        keys: Vec<Value>,
        values: Vec<Value>,
    ) -> Self {
        Value::Dictionary(Rc::new(RefCell::new(DictionaryValue {
            key_type,
            value_type,
            allow_duplicates,
            keys,
            values,
        })))
    }

    /// Compares if the Value types of this and another value are the same. This ignores values within the Value Type
    pub fn compare_valuetypes(&self, other: &Value) -> bool {
        // Simply check if they are of the same value type (NOT THE VALUES INSIDE!)
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Evaluates equality ("==") matching the three valued logic of Boolean-Likes as well
    pub fn evaluate_equal(&self, other: &Value) -> Value {
        // If a resolved possible, evaluate it and the other
        if let Some(value) = self.resolved_possible() {
            return Value::Boolean(value).evaluate_equal(other);
        }
        // Same for other-side logic
        if let Some(value) = other.resolved_possible() {
            return self.evaluate_equal(&Value::Boolean(value));
        }

        match (self, other) {
            // A very possible comparison! Possible takes the highest priority (since it could be false, and false == (possible as false) is true!)
            (Value::Possible(l_poss), r_val) => {
                let current = l_poss.borrow().current;
                if current.is_some() {
                    r_val.evaluate_equal(&Value::Boolean(l_poss.borrow().current.unwrap()))
                }
                else {
                    Value::new_possible()
                }
            }
            (l_val, Value::Possible(r_poss)) => {
                let current = r_poss.borrow().current;
                if current.is_some() {
                    l_val.evaluate_equal(&Value::Boolean(r_poss.borrow().current.unwrap()))
                }
                else {
                    Value::new_possible()
                }
            }

            // False takes next priority when comparing booleans
            (Value::Boolean(false), _) | (_, Value::Boolean(false)) => Value::Boolean(false),

            // A True comparison
            (Value::Boolean(true), Value::Boolean(true)) => Value::Boolean(true),

            // Just the normie types
            (Value::Integer(a), Value::Integer(b)) => Value::Boolean(a == b),
            (Value::Float(a), Value::Float(b)) => Value::Boolean(a == b),
            (Value::Integer(a), Value::Float(b)) => Value::Boolean((*a as f64) == *b),
            (Value::Float(a), Value::Integer(b)) => Value::Boolean(*a == (*b as f64)),

            (Value::String(a), Value::String(b)) => Value::Boolean(a == b),
            (Value::Nil, Value::Nil) => Value::Boolean(true),

            (Value::Coroutine(co1), Value::Coroutine(co2)) => Value::Boolean(Rc::ptr_eq(co1, co2)),
            (Value::Lambda(lambda1), Value::Lambda(lambda2)) => Value::Boolean(lambda1.definition.id == lambda2.definition.id),

            (Value::EnumDefinition(enum1), Value::EnumDefinition(enum2)) => Value::Boolean(enum1.identifier == enum2.identifier),
            (Value::EnumMember(mem1), Value::EnumMember(mem2)) => Value::Boolean(mem1.is_same_member(mem2)),

            _ => Value::Boolean(false),
        }
    }

    /// Evaluates inequality ("!="), with normal and possible logic
    pub fn evaluate_not_equal(&self, other: &Value) -> Value {
        if let Some(value) = self.resolved_possible() {
            return Value::Boolean(value).evaluate_not_equal(other);
        }
        if let Some(value) = other.resolved_possible() {
            return self.evaluate_not_equal(&Value::Boolean(value));
        }
        match (self, other) {
            // Possible takes highest priority for inequality too!
            (Value::Possible(l_poss), r_val) => {
                let current = l_poss.borrow().current;
                if current.is_some() {
                    r_val.evaluate_not_equal(&Value::Boolean(l_poss.borrow().current.unwrap()))
                }
                else {
                    Value::new_possible()
                }
            }
            (l_val, Value::Possible(r_poss)) => {
                let current = r_poss.borrow().current;
                if current.is_some() {
                    l_val.evaluate_not_equal(&Value::Boolean(r_poss.borrow().current.unwrap()))
                }
                else {
                    Value::new_possible()
                }
            }

            // Standard boolean inequality check
            (Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a != b),

            // Other types fall back to negating equality
            _ => match self.evaluate_equal(other) {
                Value::Boolean(b) => Value::Boolean(!b),
                value if value.is_pending_possible() => Value::new_possible(),

                _ => Value::Boolean(true),
            },
        }
    }

    /// Relational operators (>, >=, <, <=) with Possible propagation
    pub fn evaluate_relational(&self, op: &str, other: &Value) -> Value {
        // I don't know, possible is pretty weird ay?
        if let Some(value) = self.resolved_possible() {
            return Value::Boolean(value).evaluate_relational(op, other);
        }
        if let Some(value) = other.resolved_possible() {
            return self.evaluate_relational(op, &Value::Boolean(value));
        }
        // If either side is Possible, just return possible. Some say that's impossible 😂
        if self.is_pending_possible() || other.is_pending_possible() {
            return Value::new_possible();
        }

        let is_true = match (self, other) {
            // Integer and float comparisons are handled together as well!
            (Value::Integer(a), Value::Integer(b)) => match op {
                ">" => a > b,
                ">=" => a >= b,
                "<" => a < b,
                "<=" => a <= b,
                _ => false,
            },
            (Value::Float(a), Value::Float(b)) => match op {
                ">" => a > b,
                ">=" => a >= b,
                "<" => a < b,
                "<=" => a <= b,
                _ => false,
            },
            (Value::Integer(a), Value::Float(b)) => match op {
                ">" => (*a as f64) > *b,
                ">=" => (*a as f64) >= *b,
                "<" => (*a as f64) < *b,
                "<=" => (*a as f64) <= *b,
                _ => false,
            },
            (Value::Float(a), Value::Integer(b)) => match op {
                ">" => *a > (*b as f64),
                ">=" => *a >= (*b as f64),
                "<" => *a < (*b as f64),
                "<=" => *a <= (*b as f64),
                _ => false,
            },
            _ => false,
        };

        Value::Boolean(is_true)
    }

    /// Gives the runtime string name of the type the value would be
    pub fn get_runtime_typename(&self) -> Option<String> {
        match self {
            Value::String(..) => Some("StringType".into()),

            Value::Integer(..) => Some("IntegerType".into()),
            Value::Float(..) => Some("FloatType".into()),

            Value::Boolean(..) => Some("BooleanType".into()),
            // Remember, differences between if it's evaluated or not!
            Value::Possible(possible) => match possible.borrow().current {
                Some(_) => Some("BooleanType".into()),
                None => Some("PossibleType".into()),
            },

            Value::List(..) => Some("ListType".into()),
            Value::Dictionary(..) => Some("DictionaryType".into()),

            Value::Coroutine(..) => Some("CoroutineType".into()),
            Value::Lambda(..) => Some("LambdaType".into()),

            Value::EnumDefinition(enums) => Some("EnumType-".to_string() + &enums.identifier),
            Value::EnumMember(memb) => Some(memb.enum_name.clone()),

            Value::Nil => None
        }
    }

    /// Gets the type the value would be
    pub fn get_type(&self) -> MaxType {
        
        match self {
            Value::String(..) => MaxType::StringType,

            Value::Integer(..) => MaxType::IntegerType,
            Value::Float(..) => MaxType::FloatType,

            Value::Boolean(..) => MaxType::BooleanType,
            Value::Possible(possible) => match possible.borrow().current {
                Some(_) => MaxType::BooleanType,
                None => MaxType::PossibleType,
            },

            Value::List(..) => MaxType::ListType(None),
            Value::Dictionary(..) => MaxType::DictionaryType(None, None),

            Value::Coroutine(..) => MaxType::CoroutineType,
            // Keep in mind that the lambdas arity is kept within it's definition (metadata)
            Value::Lambda(lambda) => MaxType::TaskType(lambda.definition.arity as usize),

            Value::EnumDefinition(enums) => MaxType::EnumType(enums.identifier.clone()),
            Value::EnumMember(memb) => MaxType::EnumType(memb.enum_name.clone()),

            Value::Nil => MaxType::NoType
        }
    }

    /// Converts a `serde_json::Value` into a `vm::Value` type
    pub fn from_json(json: &serde_json::Value) -> Value {
        match json {
            // Numbers are as simply as pi :)
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)

                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)

                } else {
                    Value::Nil
                }
            }

            // We consider our special keywords within strings
            serde_json::Value::String(s) => match s.as_str() {
                "possible" => Value::new_possible(),
                "nil" => Value::Nil,

                _ => Value::String(s.clone()),
            },
            
            // That's just dereferencing the bool contained within
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            
            // Unwraps the object value within, e.g. {"Value": 50} or {"ResortValue": "I am very resortable"}
            serde_json::Value::Object(map) => {
                if let Some(inner_val) = map.get("Value") {
                    Self::from_json(inner_val)

                }
                else if let Some(resort) = map.get("ResortValue") {
                    Self::from_json(resort)

                }
                // Normal Object entries are treated like individual pieces of a dictionary!
                else {
                    let mut keys = Vec::new();
                    let mut values = Vec::new();

                    for (k, v) in map {
                        // Recursively parse both the key and the value
                        keys.push(Value::String(k.clone()));
                        values.push(Self::from_json(v));
                    }

                    // Return a new dictionary with no special stuff
                    Self::new_dictionary(None, None, None, keys, values)
                }
            }
            
            // Equivalent to nothing
            serde_json::Value::Null => Value::Nil,
            
            // An array is a list to us, although I wonder when we will ever send anything as an array...
            serde_json::Value::Array(arr) => {
                let items: Vec<Value> = arr.iter().map(Self::from_json).collect();

                Self::new_list(None, items)
            }
            //     element_type: None,
            //     items: arr.iter().map(Self::from_json).collect(),
            // },
        }
    }

    /// Validates that if this Value is a Dictionary, it contains no duplicate keys.
    /// Panics if duplicates are found and `allow_duplicates` is false or None.
    pub fn ensure_no_duplicate_keys(&self) {
        if let Value::Dictionary(dictionary) = self {
            let dictionary = dictionary.borrow();
            // Only enforce if allow_duplicates is false 
            if dictionary.allow_duplicates.unwrap_or(false) == false {
                let mut seen_keys: Vec<&Value> = Vec::new();

                // Move through all keys
                for key in &dictionary.keys {
                    if seen_keys.iter().any(|seen| **seen == *key) {
                        panic!("A non-duplicate dictionary has a duplicate key: {}", key);
                    }
                    seen_keys.push(key);
                }
            }
        }
    }
}

/*
    ----------------------
    ----- TYPE NAMES -----
    ----------------------
*/

/// An enum of all possible `MaxType`
#[derive(Debug, Clone, PartialEq)]
pub enum MaxType {
    NumberType,
    IntegerType,
    FloatType,

    StringType,

    BooleanType,
    PossibleType,

    ListType(Option<Box<MaxType>>),
    DictionaryType(Option<Box<MaxType>>, Option<Box<MaxType>>),

    TaskType(usize), // I.e. Lambdas
    CoroutineType,

    EnumType(String), // An EnumType has a specific identifier unique to it

    // A special type for niche situations within the code
    NoType
}

impl MaxType {    

    /// Compares if the max types of this and another type are the same. This ignores values within the Variant Type
    pub fn compare_valuetypes(&self, other: &MaxType) -> bool {
        // Like Value logic but with types
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /**
        Returns true if the `actual` type can be assigned to a variable expecting `self
        ### Example
        IntegerTypes are assignable to FloatTypes

        ```
        let integerType = MaxType::IntegerType;

        println!("{}", integerType.is_assignable_to(MaxType::FloatType)) // Prints true
        ```
    */
    pub fn is_assignable_to(&self, actual: &MaxType) -> bool {
        // When we are just comparing a type to itself
        if self == actual {
            return true;
        }

        match (self, actual) {
            // NumberType accepts both IntegerType and FloatType
            (MaxType::NumberType, MaxType::IntegerType) => true,
            (MaxType::NumberType, MaxType::FloatType) => true,

            // A FloatType can accept an IntegerType (but not vice versa)
            (MaxType::FloatType, MaxType::IntegerType) => true,

            // A possible type is also a Boolean-Like
            (MaxType::BooleanType, MaxType::PossibleType) => true,
            
            // Nested type checking for lists (E.g. A list of numbers are both ints and floats; This prevents weird type errors)
            (MaxType::ListType(Some(expected_elem)), MaxType::ListType(Some(actual_elem))) => {
                expected_elem.is_assignable_to(actual_elem)
            }

            (MaxType::DictionaryType(Some(expected_key), Some(expected_val)), MaxType::DictionaryType(Some(actual_key), Some(actual_val))) => {
                expected_key.is_assignable_to(actual_key) && expected_val.is_assignable_to(actual_val)
            }
            
            _ => false,
        }
    }

    /**
        Checks if a value and the expected type are compatible. Returns a Boolean of the result in the end.
        This function is used for runtime during Virtual Machine execution.

        ### Example
        A String Value and String Type are compatible, but not a Boolean Value and StringType.
        An Integer Value and a Number Type also works.

        ```
        let int = Value::Integer(5);

        let boolean = Value::Boolean(true);

        println!("{}", MaxType::check_value_compatibility(MaxType::IntegerType, int)); // True
        println!("{}", MaxType::check_value_compatibility(MaxType::StringType, boolean)); // False
        println!("{}", MaxType::check_value_compatibility(MaxType::NumberType, int)); // True
        ```
    */
    pub fn check_value_compatibility(expected_type: &MaxType, value: &Value) -> bool {
        match (expected_type, value) {
            (MaxType::IntegerType, Value::Integer(_)) => true,
            (MaxType::FloatType, Value::Float(_)) => true,
            (MaxType::FloatType, Value::Integer(_)) => true,
            
            // NumberType allows both Integer and Float runtime values
            (MaxType::NumberType, Value::Integer(_)) => true,
            (MaxType::NumberType, Value::Float(_)) => true,

            (MaxType::StringType, Value::String(_)) => true,

            // Possible or Boolean, it works!
            (MaxType::BooleanType, Value::Boolean(_)) => true,
            (MaxType::BooleanType, Value::Possible(_)) => true,
            (MaxType::PossibleType, Value::Possible(possible)) => possible.borrow().current.is_none(),

            (MaxType::ListType(expected_elements), Value::List(list_vals)) => {
                // This gets the elements, iterates and puts a closure upon all of them, checking if they are compatible with the element type!
                let elements_ok = match expected_elements {
                    Some(e_type) => list_vals.borrow().items.iter().all(|e| MaxType::check_value_compatibility(e_type, e)),
                    None => true,
                };
                elements_ok
            },
            
            (MaxType::DictionaryType(expected_key, expected_val), Value::Dictionary(dictionary)) => {
                let dictionary = dictionary.borrow();
                // If dictionary types are specified, check elements; if no specifications, allow any
                let key_ok = match expected_key {
                    Some(k_type) => dictionary.keys.iter().all(|k| MaxType::check_value_compatibility(k_type, k)),
                    None => true,
                };
                let val_ok = match expected_val {
                    Some(v_type) => dictionary.values.iter().all(|v| MaxType::check_value_compatibility(v_type, v)),
                    None => true,
                };
                key_ok && val_ok
            },

            (MaxType::CoroutineType, Value::Coroutine(_)) => true,
            (MaxType::TaskType(t), Value::Lambda(l)) => {
                // Check if argument count also works!
                if *t as u64 == l.definition.arity {
                    return true
                }
                false
            },

            (MaxType::EnumType(t), Value::EnumDefinition(e)) => t == &e.identifier,
            (MaxType::EnumType(t), Value::EnumMember(mem)) => t == &mem.enum_name,

            // These two are special!
            (MaxType::NoType, _) => true,
            (_, Value::Nil) => true,
            
            _ => false,
        }
    }

    /// A function that errs when the value is not compatible with the given type.
    /// Calls `MaxType::check_value_compatibility()` to do so.
    pub fn ensure_compatibility(expected_type: &MaxType, value: &Value) {
        if !MaxType::check_value_compatibility(expected_type, value) {
            panic!("Invalid Type Compatibility! Expected type '{}', got Value '{}'", expected_type, value);
        }
    }

    /// Checks if a given value is valid to be a dictionary key
    pub fn valid_dictkey_value(&self, value: &Value) -> bool {
        match self {
            // Pretty cool, '..' means just ignore what's inside
            MaxType::DictionaryType( .. ) => {
                if matches!(value, Value::String(_) | Value::Integer(_)) {
                    return true
                }

                false
            }

            _ => { println!("Can only call this function on dictionaries!"); false }
        }
    }

    /// Checks if a given type is valid to be a dictionary key
    pub fn valid_dictkey_type(&self, value: &MaxType) -> bool {
        match self {
            // Pretty cool, '..' means just ignore what's inside
            MaxType::DictionaryType( .. ) => {
                if matches!(value, MaxType::StringType | MaxType::IntegerType) {
                    return true
                }

                false
            }

            _ => { println!("Can only call this function on dictionaries!"); false }
        }
    }

    pub fn string_name_to_max(name: &str) -> MaxType {
        match name {
            "NumberType" => MaxType::NumberType,
            "IntegerType" => MaxType::IntegerType,
            "FloatType" => MaxType::FloatType,

            "StringType" => MaxType::StringType,

            "BooleanType" => MaxType::BooleanType,
            "PossibleType" => MaxType::PossibleType,

            "ListType" => MaxType::ListType(None),
            "DictionaryType" => MaxType::DictionaryType(None, None),

            "CoroutineType" => MaxType::CoroutineType,
            "TaskType" => MaxType::TaskType(0),

            "NoType" => MaxType::NoType,

            _ => panic!("Unknown MaxType name given: {}", name)
        }
    }
}

impl fmt::Display for MaxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write out the name of the Type
        match self {
            MaxType::NumberType => write!(f, "NumberType"),
            MaxType::IntegerType => write!(f, "IntegerType"),
            MaxType::FloatType => write!(f, "FloatType"),

            MaxType::StringType => write!(f, "StringType"),

            MaxType::BooleanType => write!(f, "BooleanType"),
            MaxType::PossibleType => write!(f, "PossibleType"),

            MaxType::NoType => write!(f, "NoType"),

            // Lists can also write out their element type
            MaxType::ListType(elem) => match elem {
                Some(inner) => write!(f, "ListType<{}>", inner),

                None => write!(f, "ListType"),
            },

            // Dictionaries can also write out the key/value types
            MaxType::DictionaryType(key, val) => match (key, val) {
                (Some(k), Some(v)) => write!(f, "DictionaryType<{}, {}>", k, v),

                _ => write!(f, "DictionaryType"),
            },

            MaxType::CoroutineType => write!(f, "CoroutineType"),
            MaxType::TaskType(_) => write!(f, "TaskType"),

            MaxType::EnumType(enum_name) => write!(f, "{}", enum_name)
        }
    }
}

// Doing this allows us to convert the string from the JSON into a MaxType immediately
impl<'de> Deserialize<'de> for MaxType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MaxTypeVisitor;

        // Here we find the MaxType 'visitor' to convert into valid Typing
        impl<'de> Visitor<'de> for MaxTypeVisitor {
            type Value = MaxType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object containing type representation metadata")
            }

            // This allows handling null values
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaxType::NoType)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaxType::NoType)
            }

            // Just for when the type name is a simply string value
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaxType::string_name_to_max(value))
            }

            // We have both a visit_str and visit_map so that types, whether as JSON objects or strings, can always be parsed!
            fn visit_map<M>(self, mut map: M) -> Result<MaxType, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut kind: Option<String> = None;
                let mut name: Option<String> = None;
                let mut element_type: Option<MaxType> = None;
                let mut key_type: Option<MaxType> = None;
                let mut value_type: Option<MaxType> = None;

                // Loop through keys in the JSON map
                while let Some(key) = map.next_key::<String>()? {
                    // This is where we check inside the 'Type' field
                    match key.as_str() {
                        "kind" => kind = Some(map.next_value()?),
                        "name" => name = Some(map.next_value()?),

                        "elementType" => element_type = Some(map.next_value()?),
                        "keyType" => {
                            // Allows null JSON values to deserialize as None instead of throwing an error
                            key_type = map.next_value::<Option<MaxType>>()?;
                        }
                        "valueType" => {
                            value_type = map.next_value::<Option<MaxType>>()?;
                        }

                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                // Check if kind exists (it must!)
                let kind_str = kind.ok_or_else(|| de::Error::missing_field("kind"))?;

                // And now we check for what type it is
                match kind_str.as_str() {

                    // Primitive types as most are. They only hold a 'name' field
                    "PrimitiveType" => {
                        let type_name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                        Ok(MaxType::string_name_to_max(&type_name))
                    }

                    // Enum types are custom
                    "EnumType" => {
                        let enum_name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                        Ok(MaxType::EnumType(enum_name))
                    }

                    // Iterables hold types for their respective elements too! \\

                    "ListType" => Ok(MaxType::ListType(element_type.map(Box::new))),

                    "DictionaryType" => Ok(MaxType::DictionaryType(
                        key_type.map(Box::new),
                        value_type.map(Box::new),
                    )),

                    _ => Err(de::Error::custom(format!("Unknown type kind: {}", kind_str))),
                }
            }
        }

        deserializer.deserialize_any(MaxTypeVisitor)
    }
}


/*
    -----------------------
    --- VIRTUAL MACHINE ---
    -----------------------
*/


/// The tracker for task calls and their ip locations, variables and metadata
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// The name of the task being executed
    pub task_info: Option<TaskInfo>,

    /// The (optional) coroutine instance that may be attached to this call. This is independent of `task_info`
    pub coroutine_instance: Option<CoroutineHandle>,

    /// The saved instruction point to jump back to upon returning
    pub return_ip: usize,

    /// The instruction chunk to resume after this call returns.
    pub return_instructions: Arc<Vec<Instruction>>,

    pub is_lambda: bool,

    /// The base index on the stack on where this frame's arguments and local scoping starts
    pub scope_depth: usize,
    pub module_name: String
}

impl CallFrame {
    pub fn new(task_info: Option<TaskInfo>, coroutine_instance: Option<CoroutineHandle>, return_ip: usize,
                return_instructions: Arc<Vec<Instruction>>, is_lambda: bool, scope_depth: usize, module_name: String) -> Self
    {
        Self { task_info, coroutine_instance, return_ip, return_instructions, is_lambda, scope_depth, module_name }
    }
}

pub struct VirtualMachine {
    stack: Vec<Value>,
    scopes: Vec<Arc<HashMap<String, VariableEntry>>>,
    temporaries: HashMap<u64, Value>,

    call_stack: Vec<CallFrame>,
    native_registry: NativeRegistry,

    pub active_instructions: Arc<Vec<Instruction>>,

    task_definitions: Arc<HashMap<String, TaskInfo>>,
    modules: Arc<HashMap<String, Bytecode>>,
    module_name: String,
    enums: Arc<HashMap<String, EnumStructure>>,
    lambda_cache: HashMap<u64, Arc<LambdaOperand>>,

    /// A collector for the dependencies the current possible handles hold
    dependency_collector: Option<Rc<RefCell<HashSet<DependencyKey>>>>,
    /// A Hashmap of subscribers by possible signals
    dependency_subscribers: HashMap<DependencyKey, Vec<Weak<RefCell<PossibleSignal>>>>,
    /// A Hashset of the possible signals being evaluated via their ids
    evaluating_possible: HashSet<usize>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            scopes: vec![Arc::default()],
            temporaries: HashMap::new(),
            call_stack: vec![],
            native_registry: NativeRegistry::new(),
            active_instructions: Arc::default(),
            task_definitions: Arc::default(),
            modules: Arc::default(),
            module_name: String::new(),
            enums: Arc::default(),
            lambda_cache: HashMap::new(),
            dependency_collector: None,
            dependency_subscribers: HashMap::new(),
            evaluating_possible: HashSet::new(),
        }
    }

    /// The heart of the VM and what allows it to execute all bytecode instructions
    /// Must have a Bytecode param passed in
    pub fn run(&mut self, module_name: String, bytecode: Bytecode) {
        self.run_chunk(module_name, bytecode, true);
    }

    /// Begins executing the first 'main' file found, starting from instruction zero
    pub fn run_main(&mut self) {
        // Start execution by fetching "main" from the modules map
        let main_module = self.modules.get("main").cloned().expect("No main.max file inside modules found!");

        // Begin execution loop starting at instruction 0 in mainD
        self.run_chunk(String::from("main"), main_module, false);
    }

    /// The function that runs through any bytecode given.
    /// Although it's used to run the first bytecode that is given,
    /// it can also be used to run other seperate bytecode chunks.
    /// E.g. from a lambda, check out the VM's `call_lambda` function.
    fn run_chunk(&mut self, module_name: String, bytecode: Bytecode, print_completion: bool) {
        self.module_name = module_name;

        self.task_definitions = Arc::clone(&bytecode.tasks); // Give ourselves the current tasks defined
        // The main instructions that were given to us by the json
        let main_instructions = Arc::clone(&bytecode.instructions);

        /*
            The active instructions that are currently being executed.
            These instructions are mutable, meaning they can change around.
            However they should ALWAYS fallback to the main instructions when they are finished!
        */
        let mut active_instructions = Arc::clone(&main_instructions);
        self.active_instructions = Arc::clone(&active_instructions);

        let mut ip = 0; // The instruction pointer

        // We use a while loop in order to allow ip to be mutable and changed within the loop!
        while ip < active_instructions.len() {
            let instr = active_instructions[ip].clone();

            match instr.op_code.as_str() {
                "Constant" => {
                    // Get value
                    if let Some(val) = &instr.operand {
                        // Get as some different types of values
                        if let Some(s) = val.as_str() {
                            // What if it is not just ANY string? hmmmmm?
                            match s {
                                "possible" => self.stack.push(Value::new_possible()),
                                
                                _ => self.stack.push(Value::String(s.to_string())),
                            }
                        }
                        else if let Some(n) = val.as_i64() {
                            self.stack.push(Value::Integer(n));
                        }
                        else if let Some(n) = val.as_f64() {
                            self.stack.push(Value::Float(n));
                        } else if let Some(b) = val.as_bool() {
                            self.stack.push(Value::Boolean(b));
                        } else if let Some(_) = val.as_null() {
                            self.stack.push(Value::Nil);
                        }
                    }
                }

                "MakeList" => {
                    if let Some(count) = &instr.operand && let Some(val) = count.as_u64() {
                        let mut listables: Vec<Value> = vec![];

                        for _index in 0..val {
                            if let Some(object) = self.stack.pop() {
                                // Insert at the beginning
                                listables.insert(0, object);
                            }
                        }

                        self.stack.push(Value::new_list(None, listables));
                    }
                }

                "MakeDictionary" => {
                    if let Some(operand) = &instr.operand {
                        let entry_count = operand.as_u64().unwrap_or(0);

                        let mut keys_list: Vec<Value> = vec![];
                        let mut values_list: Vec<Value> = vec![];

                        for _count in 0..entry_count {
                            // We pop value first because we are starting from the top of the stack. Remember?
                            let val = self.stack.pop().unwrap_or(Value::Nil);
                            let key = self.stack.pop().unwrap_or(Value::Nil);

                            // Validate typing
                            DictionaryType(None, None).valid_dictkey_value(&key);

                            // We insert it at the beginning, because AGAIN!
                            // We are taking values from popping the stack, which is in reverse order!
                            keys_list.insert(0, key);
                            values_list.insert(0, val);
                        }

                        let dictionary = Value::new_dictionary(None, None, None, keys_list, values_list);
                        dictionary.ensure_no_duplicate_keys();

                        self.stack.push(dictionary);
                    }
                }

                "CreateCoroutine" => {
                    if let Some(operand_json) = &instr.operand {
                        let coroutine_info: CoroutineInfo = serde_json::from_value(operand_json.clone())
                            .expect("Invalid Create-Coroutine operand format");

                        let inst: CoroutineInstance
                            = CoroutineInstance { execution_state: ExecutionState::Idle, return_type: coroutine_info.return_type,
                                                  instruction_point: coroutine_info.entry_point, entry_point: coroutine_info.entry_point,
                                                  saved_scopes: None, module_name: self.module_name.clone() };

                        self.stack.push(Value::Coroutine(Rc::new(RefCell::new(inst))));
                    } else {
                        panic!("[IP {:04}] Operand for creating a coroutine must include it's necessary info.", ip)
                    }
                }

                "DefineEnum" => {
                    if let Some(operand_json) = &instr.operand {
                        let enum_info: EnumStructure = serde_json::from_value(operand_json.clone())
                            .expect("Invalid Define-Enum operand format");

                        let bound = enum_info.identifier.clone();

                        // Push in the new enum
                        // Ye that's it
                        Arc::make_mut(&mut self.enums).insert(bound, enum_info);
                    }
                }

                // field: lambda_cache: HashMap<u64, Arc<LambdaOperand>>

                "Lambda" => {
                    if let Some(operand_json) = &instr.operand {
                        let id = operand_json.get("ID").and_then(|v| v.as_u64())
                            .expect("Lambda operand is missing its ID");

                        // Very nicely get the lambda found from the VM Hashmap
                        // Inserting a new lambda into the cache if it doesn't already exist
                        let definition = match self.lambda_cache.get(&id) {
                            Some(cached) => Arc::clone(cached),
                            None => {
                                let parsed: Arc<LambdaOperand> = Arc::new(
                                    LambdaOperand::deserialize(operand_json)
                                        .expect("Invalid LambdaOperand attached to Lambda Instruction"));
                                self.lambda_cache.insert(id, Arc::clone(&parsed));
                                parsed
                            }
                        };

                        let mut captures = HashMap::new();
                        // Input them captures!
                        for name in &definition.captured {
                            let entry = self.scopes.iter().rev().find_map(|scope| scope.get(name));
                            if let Some(entry) = entry {
                                captures.insert(name.clone(), entry.clone());
                            } else {
                                panic!("[IP {:04}] Captured variable '{}' is not defined.", ip, name);
                            }
                        }

                        // Push, as always
                        self.stack.push(Value::Lambda(RuntimeLambda {
                            definition,
                            captures: Arc::new(captures),
                        }));
                    }
                }

                "MemberAccess" => {
                    if let Some(operand_json) = &instr.operand {
                        let operand: MemberAccessOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid MemberAccessOperand format");

                        // Match based on the access type sent by the compiler ("Module", "Enum", etc.)
                        match operand.access_type.as_str() {
                            "Module" => {
                                // Let us pop the module name like a balloon off the stack
                                let target_val = self.stack.pop().unwrap_or(Value::Nil);
                                let module_name = match target_val {
                                    Value::String(s) => s,
                                    _ => panic!("[IP {:04}] Expected module name string on stack, got {:?}", ip, target_val),
                                };

                                // Look up the imported module
                                if let Some(module_bytecode) = self.modules.get(&module_name) {
                                    // Check if the member is an enum inside this module
                                    if let Some(enum_info) = module_bytecode.enums.get(&operand.member_name).cloned() {
                                        let enum_def = Value::EnumDefinition(enum_info);
                                        
                                        // Now, push the resolved EnumDefinition onto the stack!
                                        self.stack.push(enum_def);
                                    } 
                                    // Twas a coroutine instead?
                                    else if let Some(coroutine_info) = module_bytecode.coroutines.get(&operand.member_name) {
                                        let instance = CoroutineInstance {
                                            execution_state: ExecutionState::Idle,
                                            return_type: coroutine_info.return_type.clone(),
                                            instruction_point: coroutine_info.entry_point,
                                            entry_point: coroutine_info.entry_point,
                                            saved_scopes: None,
                                            module_name: module_name.clone(),
                                        };
                                        self.stack.push(Value::Coroutine(Rc::new(RefCell::new(instance))));
                                    } 
                                    else {
                                        panic!(
                                            "[IP {:04}] Member '{}' not found in imported module '{}'",
                                            ip, operand.member_name, module_name
                                        );
                                    }
                                } else {
                                    panic!("[IP {:04}] Module '{}' not found", ip, module_name);
                                }
                            }

                            "Enum" => {
                                // Else pop the EnumDefinition that was pushed by the previous access
                                let target_val = self.stack.pop().unwrap_or(Value::Nil);
                                let enum_struct = match target_val {
                                    Value::EnumDefinition(e) => e,
                                    _ => panic!("[IP {:04}] Expected EnumDefinition on stack for enum member access, got {:?}", ip, target_val),
                                };

                                // Look up the specific member (e.g., "North") inside the enum
                                if let Some(member_value) =
                                    enum_struct.enum_members.into_iter().find(|mem| mem.name == operand.member_name)
                                {
                                    // Push the final resolved enum member value onto the stack
                                    self.stack.push(Value::EnumMember(member_value));
                                }
                                else {
                                    panic!("[IP {:04}] No member found for enum '{}' by the name '{}'", ip, enum_struct.identifier, operand.member_name)
                                }
                            }

                            "EnumMember" => {
                                let target_val = self.stack.pop().unwrap_or(Value::Nil);
                                // Get the parent. It MUST be an EnumMember... for obvious reasons
                                let parent = match target_val {
                                    Value::EnumMember(m) => m,
                                    _ => panic!("[IP {:04}] Expected an enum member on the stack, got {:?}", ip, target_val),
                                };

                                // Now get the sub-member
                                let child = self.find_sub_member(&parent, &operand.member_name)
                                    .unwrap_or_else(|| panic!(
                                        "[IP {:04}] '{}' has no sub member '{}'",
                                        ip, parent.full_name(), operand.member_name
                                    ));

                                // Push!!!
                                self.stack.push(Value::EnumMember(child));
                            }

                            _ => {
                                panic!("[IP {:04}] Unknown member access type: '{}'", ip, operand.access_type);
                            }
                        }
                    }
                }

                // Legit... It just pops from the stack
                "Pop" => { self.stack.pop(); }


                /*
                 -----------------
                 --- VARIABLES ---
                 -----------------
                */

                
                // Push a new local variable scope onto the stack
                "EnterScope" => {
                    self.scopes.push(Arc::new(HashMap::new()));
                }

                // Pop the innermost scope, destroying its local variables along with it
                "ExitScope" => {
                    if self.scopes.len() > 1 {
                        self.scopes.pop();
                    } else {
                        // Error for the global scope
                        eprintln!("[IP {:04}] Attempted to exit global scope!", ip);
                    }
                }

                // Store a variable within the most local scope concurrently
                "Store" => {
                    if let Some(operand_json) = &instr.operand {
                        let var_info: StoreOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid Store operand format");
                        let mut val = self.stack.pop().unwrap_or(Value::Nil);

                        // Get resort value if possible without moving the option out of var_info. (aka not stealing the child from it's mother)
                        let resort_value = var_info.resort_value.as_ref();
                        
                        // We map the json to get any value within (which for the resortValue would the "Value", look at any json files to see this!)
                        let resort_val: Option<Value> = resort_value.map(Value::from_json);

                        if &val == &Value::Nil && let Some(resortable) = resort_val.clone() {

                            // Yep, checking for this as well!
                            if let Some(resort_checkable) = &var_info.max_type {
                                MaxType::ensure_compatibility(resort_checkable, &resortable);
                            }

                            val = resortable;
                        }

                        // Lotta type checking!
                        if let Some(type_checkable) = &var_info.max_type {
                            MaxType::ensure_compatibility(type_checkable, &val);
                        }

                        let entry = VariableEntry::new(
                            val, var_info.modifiers, var_info.max_type, resort_val,
                        );
                        
                        let existing = self.lookup_variable(&var_info.name);

                        /*
                            Does the found variable (already declared) allow shadowing?
                            Be aware that this finds the FIRST existing variable, so if it finds a variable shadowing another variable...
                            And THAT variable does not have shadowing allows, then this fails!
                        */ 
                        if let Some(found) = existing && !found.modifiers.contains(&String::from("open")) {
                            panic!("Cannot redeclare a variable! Modify the original variable to be open if needed.")
                        }

                        let current_scope = Arc::make_mut(self.scopes.last_mut().expect("Scopes stack is empty!"));

                        current_scope.insert(var_info.name, entry);
                    }
                },

                "TemporaryStore" => {
                    // Check for value popped and index in operand
                    if let Some(val) = &self.stack.pop() {
                        if let Some(operand_json) = &instr.operand {
                            let slot = operand_json.as_u64().expect("Slot for a Temporary Store must be a positive integer");

                            // Now insert!
                            self.temporaries.insert(slot, val.clone());
                        }
                        else {
                            panic!("[IP {:04}] Operand was not given for temporarily storing.", ip);
                        }
                    }
                    else {
                        panic!("[IP {:04}] No value found to store in temporaries.", ip);
                    }
                }
                
                // Load a variable from the current scope or parent scopes
                // Scoping prevents scopes that were destroyed from being called
                "Load" => {
                    if let Some(operand_json) = &instr.operand {
                        // Get string name
                        let name = operand_json.as_str()
                            .expect("The operand of a 'Load' must be a string name");
                        let mut found_binding: Option<Rc<RefCell<Value>>> = None;

                        // Check through each scope, and when the name is found, break with the new value
                        // Btw, reverse the scopes to start from the innermost one!
                        if let Some(found_var) = self.lookup_variable(name) {
                            found_binding = Some(Rc::clone(&found_var.value));
                        }

                        // Update here
                        if let Some(binding) = found_binding {
                            // This whole process if way more complicated than you think due to possible handle dependencies
                            // This checks if a dependency upon the variable value was placed
                            self.track_dependency(DependencyKey::Binding(Rc::as_ptr(&binding) as usize));

                            let val = binding.borrow().clone();
                            match &val {
                                // Same here for lists and dictionaries, though they got their own kind of Dependency Key
                                Value::List(list) => self.track_dependency(DependencyKey::List(Rc::as_ptr(list) as usize)),
                                Value::Dictionary(dictionary) => self.track_dependency(DependencyKey::Dictionary(Rc::as_ptr(dictionary) as usize)),

                                // NOTE: Enums can't be changed after being defined, so no dependencies are ever placed upon them

                                _ => {}
                            }

                            if let Value::Possible(possible) = &val {
                                // If the possible is not evalutaed, place a dependency tracker upon it too
                                if possible.borrow().current.is_none() {
                                    self.track_dependency(DependencyKey::Possible(Rc::as_ptr(possible) as usize));
                                    self.stack.push(val);
                                }
                                else {
                                    // Push the possible. We'll have to unfortunately unwrap it ourselves else where
                                    self.stack.push(Value::Possible(possible.clone()));
                                }
                            }
                            else {
                                // In the end, it will be pushed into the stack!
                                self.stack.push(val);
                            }
                        }
                        // Check if it's an imported module
                        else if self.modules.contains_key(name) {
                            // Push a string value of the name onto the stack
                            self.stack.push(Value::String(name.to_string()));
                            ip += 1;
                            continue;
                        }
                        else {
                            // No variable found? Let's check enums now

                            let mut found: Option<(&String, &EnumStructure)> = None;

                            for enumerator in self.enums.iter() {
                                if name == &enumerator.1.identifier {
                                    found = Some(enumerator);
                                }
                            }

                            if let Some(enum_found) = found {
                                let enum_value: Value = Value::EnumDefinition(enum_found.1.clone());

                                self.stack.push(enum_value);
                            } else {
                                panic!("[IP {:04}] Variable or Enum '{}' is not defined in any active scope.", ip, name);
                            }

                        }
                    }
                }

                "TemporaryLoad" => {
                    // Check the operand
                    if let Some(operand_json) = &instr.operand {
                        let slot = operand_json.as_u64().expect("Slot for a Temporary Load must be a positive integer");

                        // Get object
                        let object = self.temporaries.get(&slot);

                        if let Some(obj) = object {
                            // Push as a CLONE
                            self.stack.push(obj.clone());
                        } else {
                            panic!("[IP {:04}] Attempted to load undefined temporary object of slot {}.", ip, slot)
                        }
                    }
                }

                // Search from innermost to outermost scope and update an existing variable
                "Update" => {
                    if let Some(operand_json) = &instr.operand {
                        let var_info: UpdateOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid Update operand format");
                        let new_val = self.stack.pop().unwrap_or(Value::Nil);
                        let mut updated = false;
                        let mut changed_binding = None;

                        for scope in self.scopes.iter_mut().rev() {
                            if scope.contains_key(&var_info.name) {
                                let entry = Arc::make_mut(scope)
                                    .get_mut(&var_info.name)
                                    .expect("The scope was checked to contain a variable entry, but an error occured");

                                // A fixed variable? Gotta fix this one!
                                if entry.modifiers.contains(&String::from("fixed")) {
                                    panic!("Cannot modify a fixed variable")
                                }

                                // Type safety
                                if let Some(ensurable) = &entry.max_type {
                                    MaxType::ensure_compatibility(ensurable, &new_val);
                                }

                                // Easy reference ip here
                                let easy_ip_ref = Some(Value::Integer(ip as i64));

                                // The variations of updating here too
                                let old_value = entry.value.borrow().clone();
                                let updated_value = match var_info.operation.as_str() {
                                    "=" => new_val,
                                    "+=" => VirtualMachine::add_values(&old_value, &new_val, easy_ip_ref),
                                    "-=" => VirtualMachine::subtract_values(&old_value, &new_val, easy_ip_ref),
                                    "*=" => VirtualMachine::multiply_values(&old_value, &new_val, easy_ip_ref),
                                    "/=" => VirtualMachine::divide_values(&old_value, &new_val, easy_ip_ref),
                                    "%=" => Self::modulo_values(&old_value, &new_val, easy_ip_ref),

                                    _ => panic!("Unknown operation for updating given: {}", var_info.operation)
                                };

                                // Also check for the resort value if needed
                                let final_val = if updated_value == Value::Nil {
                                    entry.resort_value.clone().unwrap_or(Value::Nil)
                                } else {
                                    updated_value
                                };

                                // Don't worry, the function itself allows duplicates when the dictionary says so
                                final_val.ensure_no_duplicate_keys();

                                // Set the value as the updated, blah blah.
                                *entry.value.borrow_mut() = final_val;
                                // Create a binding to this updated value, so we can update our dependency trackers via
                                changed_binding = Some(Rc::as_ptr(&entry.value) as usize);

                                updated = true;
                                break;
                            }
                        }

                        if !updated {
                            panic!("[IP {:04}] Cannot update undefined variable '{}'", ip, var_info.name);
                        }

                        // Check em trackers like some kind of dog collar!
                        if let Some(binding_id) = changed_binding {
                            self.notify_dependency(DependencyKey::Binding(binding_id));
                        }
                    }
                }

                "Index" => {
                    // Index value -> Target to index
                    if let Some(index) = self.stack.pop() {
                        if let Some(target) = self.stack.pop() {
                            // Don't worry! We can use the Values directly without referencing because, after popping them, they'll be dropped anyways!
                            let evaluated = Self::eval_indexing(index, target);

                            self.stack.push(evaluated);
                        }
                    }
                }

                "UpdateIndex" => {
                    // We pop the values in this order
                    // Value to update with -> Index value -> Target to index and update

                    // Clean way of popping the stack thrice
                    match (self.stack.pop(), self.stack.pop(), self.stack.pop()) {
                        (Some(value), Some(index), Some(target)) => {
                            if let Some(operand_json) = &instr.operand {
                                let operand: UpdateIndexOperand = serde_json::from_value(operand_json.clone())
                                    .expect("Invalid UpdateIndexOperand");

                                let observed_target = target.clone();
                                let updated_target = VirtualMachine::eval_update_indexing(index, target, value, &operand.operation)
                                                                .expect("Evaluation of updating an index expected an error.");

                                // Keep in mind that indexing only applies to collections, so we just run this!
                                self.notify_collection_changed(&observed_target);
                                                            
                                if let Some(target_name) = operand.target_name {
                                    self.update_variable_value(target_name, updated_target);
                                }
                            }
                        }

                        // Ensures only when all three are present, it will continue
                        _ => {
                            println!("[IP {:04}] Not enough items in stack whilst updating an index", ip);
                        }
                    }
                }

                "ForeachNext" => {
                    if let Some(operand_json) = &instr.operand {
                        let operand: ForeachOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid ForeachNext operand format");

                        // Attempt to get the temporary index slot
                        let index = match self.temporaries.get(&operand.index_slot) {
                            Some(Value::Integer(i)) => *i as usize,
                            _ => panic!("Foreach index was not initialized"),
                        };

                        // In the collection slot (where the iterable is), attempt to borrow it's count and get the item from the current index (the start)
                        let (item_count, current_item) = match self.temporaries.get(&operand.collection_slot) {
                            Some(Value::List(list)) => {
                                let list = list.borrow();
                                (list.items.len(), list.items.get(index).cloned())
                            }

                            Some(Value::Dictionary(dictionary)) => {
                                let dictionary = dictionary.borrow();
                                if dictionary.keys.len() != dictionary.values.len() {
                                    panic!("Key/Value count mismatch in dictionary iteration.");
                                }
                                
                                // This *simply* gets all the keys and values, and maps them into a list of key-value pairs!
                                let current_item = dictionary.keys
                                    .get(index).zip(dictionary.values.get(index))
                                    .map(|(key, value)| Value::new_list(None, vec![key.clone(), value.clone()]));

                                (dictionary.keys.len(), current_item)
                            }

                            Some(Value::String(str)) => {
                                // Remember to return the character count, not bits length!
                                let char_found = str.chars().nth(index).unwrap_or(' ');
                                (str.chars().count(), Some(Value::String(char_found.to_string())))
                            }

                            _ => panic!("Can only iterate over an iterable collection"),
                        };

                        if index >= item_count {
                            // We finished? Push false to signify the end of the loop
                            self.stack.push(Value::Boolean(false));
                        } else {
                            // Erring is more useful than unwrapping a default
                            let current_item = current_item.expect("Foreach item should exist for an in-range index");

                            // --- MULTI-VARIABLE UNPACKING ---
                            // We go through each declaration item
                            for (var_idx, var_name) in operand.declarations.iter().enumerate() {
                                // Determine the value for this specific variable
                                let val_to_assign = match &current_item {

                                    // If the current item is a List/Tuple, extract the matching element index (determined by the for loop!)
                                    Value::List(inner_list) => {
                                        let inner = inner_list.borrow();
                                        inner.items.get(var_idx).cloned().unwrap_or(Value::Nil)
                                    }

                                    // Otherwise assign the raw item to the first variable
                                    _ => if var_idx == 0 { current_item.clone() } else { Value::Nil }
                                };

                                // Scope resolution & Variable assignment
                                let mut var_scope_idx = None;
                                for (idx, scope) in self.scopes.iter().enumerate().rev() {
                                    if scope.contains_key(var_name) {
                                        var_scope_idx = Some(idx);
                                        break;
                                    }
                                }

                                let scope_idx = var_scope_idx.unwrap_or_else(|| {
                                    panic!("Variable was not defined yet: {}", var_name)
                                });

                                let mut_scope = Arc::make_mut(&mut self.scopes[scope_idx]);

                                // Set either to the variable if it's already been declared, or create a new one for it
                                let changed_binding = if let Some(entry) = mut_scope.get_mut(var_name) {
                                    *entry.value.borrow_mut() = val_to_assign;
                                    Some(Rc::as_ptr(&entry.value) as usize)
                                } else {
                                    mut_scope.insert(
                                        var_name.clone(),
                                        // BTW, you'll especially see empty modifiers given to Variable entries, as they only apply to declarations
                                        VariableEntry::new(val_to_assign, vec![], operand.variable_type.clone(), None),
                                    );
                                    None
                                };

                                // Notify dependencies for reactive/possible bindings
                                if let Some(binding_id) = changed_binding {
                                    self.notify_dependency(DependencyKey::Binding(binding_id));
                                }
                            }

                            // Increment loop index in temporaries
                            self.temporaries.insert(
                                operand.index_slot,
                                Value::Integer((index + 1) as i64),
                            );

                            // Once finished with this, we push true to signify the continuation of the loop!
                            self.stack.push(Value::Boolean(true));
                        }
                    }
                    // Panicking to help us identify errors
                    else {
                        panic!("Attempted to get a ForeachNext Operand, got None")
                    }
                }

                // Legit just say something. Like what else
                "Say" => {
                    if let Some(val) = &self.stack.pop() {
                        println!("{}", val);
                    }
                }


                /*
                   -------------
                    ARITHMETICS
                   -------------
                */


                "Add" => {
                    // We are popping from the stack, so the right value comes first!
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    let result = VirtualMachine::add_values(&left, &right, Some(Value::Integer(ip as i64)));

                    self.stack.push(result);
                }

                "Subtract" => {
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    let result = VirtualMachine::subtract_values(&left, &right, Some(Value::Integer(ip as i64)));

                    self.stack.push(result);
                }

                "Multiply" => {
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    let result = VirtualMachine::multiply_values(&left, &right, Some(Value::Integer(ip as i64)));

                    self.stack.push(result);
                }

                "Divide" => {
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    let result = VirtualMachine::divide_values(&left, &right, Some(Value::Integer(ip as i64)));

                    self.stack.push(result);
                }

                "Modulus" => {
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    let result = VirtualMachine::modulo_values(&left, &right, Some(Value::Integer(ip as i64)));

                    self.stack.push(result);
                }

                "Compare" => {
                    let right = self.stack.pop().unwrap_or(Value::Nil);
                    let left = self.stack.pop().unwrap_or(Value::Nil);

                    if let Some(operator) = &instr.operand {
                        // Represent as  string, else just blank
                        let str_oper = operator.as_str().unwrap_or("");

                        let result = Self::eval_comparison(str_oper, left, right);

                        // Use evaluated value to push in the boolean(-like)!
                        self.stack.push(result);
                    }
                }


                /*
                 -----------------
                 ----- LOGIC -----
                 -----------------
                */


                "Not" => {
                    if let Some(val) = self.stack.pop() {
                        // Simply inverse value depending on what value we are performing upon
                        match val {
                            Value::Boolean(boolean) => self.stack.push(Self::negate_boollike(&Value::Boolean(boolean))),

                            Value::Possible(possible) => {
                                if let Some(value) = possible.borrow().current {
                                    let new = Self::negate_boollike(&Value::Boolean(value));
                                    self.stack.push(new);
                                }
                                else {
                                    self.stack.push(Value::new_possible());
                                }
                            }
                            
                            _ => panic!("Can only perform 'not' inversing upon boolean-evaluated values!")
                        }
                    }
                }

                "Negate" => {
                    if let Some(val) = self.stack.pop() {
                        // For now, negation only applies to numbers
                        match val {
                            Value::Float(fl) => self.stack.push(Value::Float(-fl)),
                            Value::Integer(int) => self.stack.push(Value::Integer(-int)),

                            _ => panic!("Can only perform negation on integers and floats")
                        }
                    }
                }


                /*
                 -------------------
                 ----- CALLING -----
                 -------------------
                */


                "Call" => {
                    if let Some(operand_json) = &instr.operand {
                        let call: TaskCallOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid TaskCallOperand attached to Call Instruction!");

                        // If there is no target, we search through the scopes to check if a lambda of matching name exists
                        // From there, lambda is given a lambda value. Otherwise None
                        let lambda = if call.target.is_none() {
                            self.scopes.iter().rev().find_map(|scope| {
                                scope.get(&call.name).and_then(|entry| match &*entry.value.borrow() {
                                    Value::Lambda(lambda) => Some(lambda.clone()),
                                    
                                    _ => None,
                                })
                            })
                        } else {
                            None
                        };

                        // If a lambda exists
                        if let Some(lambda) = lambda {
                            // Using it's metadata, we ensure the number of arguments is equal to the lambdas parameter count (arity)
                            let definition = &lambda.definition;
                            if call.argument_count != definition.arity {
                                panic!("Lambda '{}' expects {} arguments, but {} were provided", call.name, definition.arity, call.argument_count);
                            }

                            // Get the arguments
                            let mut arguments = Vec::with_capacity(call.argument_count as usize);
                            for _ in 0..call.argument_count {
                                arguments.push(self.stack.pop().unwrap_or_else(|| panic!("Not enough arguments provided to lambda '{}'", call.name)));
                            }
                            arguments.reverse(); // You should know why by now...

                            let parameter_names = if definition.parameters.len() == definition.arity as usize {
                                // When the parameters are explicitly defined, simply clone them
                                definition.parameters.clone()
                            }
                            else if definition.parameters.is_empty() && definition.arity == 1 {
                                /*
                                    If the parameter was not declared explicitly, but one parameter was expected
                                    Look through the instructions to find the first 'Load' instruction
                                    From the load instruction, obtain the name of the Load Instruction (the operand)
                                    Assign that name as the parameter name!
                                */
                                definition.chunk.instructions.iter().find_map(|instruction| {
                                    if instruction.op_code == "Load" {
                                        instruction.operand.as_ref()
                                            .and_then(serde_json::Value::as_str)
                                            .filter(|name| !lambda.captures.contains_key(*name))
                                            .map(str::to_owned)
                                    }
                                    else {
                                        // Or not I guess
                                        None
                                    }
                                }).into_iter().collect() // Put into a Vector
                            }
                            else {
                                panic!("Lambda '{}' must provide parameter names for its arity", call.name);
                            };

                            // Check!
                            if parameter_names.len() != arguments.len() {
                                panic!("Could not determine parameter names for lambda '{}'", call.name);
                            }

                            let mut parameters: HashMap<String, VariableEntry> = HashMap::new();

                            for (name, argument) in parameter_names.into_iter().zip(arguments) {
                                // All the parameters inputted are also inserted into the callscope as declared variables
                                parameters.insert(name, VariableEntry::new(
                                    argument, vec![], None, None )
                                );
                            }

                            // Push into the callstack a new LAMBDA call
                            // This is identifiable by how there is no task info, and the is_lambda bool is marked true!
                            self.call_stack.push(CallFrame {
                                task_info: None,
                                coroutine_instance: None,
                                return_ip: ip + 1,
                                return_instructions: Arc::clone(&active_instructions),
                                is_lambda: true,
                                scope_depth: self.scopes.len(),
                                module_name: self.module_name.clone()
                            });
                            // We push the captures and the params as seperate scopes
                            // Don't worry, the scope-depth cleanup logic will take them both out!
                            self.scopes.push(lambda.captures);
                            self.scopes.push(Arc::new(parameters));

                            // We set the current ACTIVE instructions as the lambda chunks instructions
                            // Keep in mind that when the lambda is finished, the instructions return to the normal bytecode!
                            active_instructions = definition.chunk.instructions.clone();
                            self.active_instructions = Arc::clone(&active_instructions);
                            ip = 0;

                            continue;
                        }
                        
                        // Calling a user defined task via a module name
                        if let Some(ref target_module_name) = call.target {
                            if let Some(task) = bytecode.tasks.get(&call.name).cloned() {
                                self.invoke_task(
                                    task,
                                    target_module_name,
                                    call.argument_count,
                                    &mut ip,
                                    &mut active_instructions,
                                );
                                continue;
                            }

                            // Fallback to Native Module Call if target isn't a user module or task wasn't found
                            let mut control_transferred = false;
                            let native_res = self.invoke_native(
                                call,
                                &mut ip,
                                &mut active_instructions,
                                &mut control_transferred,
                            );

                            if control_transferred {
                                continue;
                            }
                            self.stack.push(native_res);
                            ip += 1;
                            continue;
                        }

                        // Local Task in the same file
                        if let Some(task) = self.task_definitions.get(&call.name).cloned() {
                            self.invoke_task( 
                                task,
                                &self.module_name.to_string(),
                                call.argument_count,
                                &mut ip,
                                &mut active_instructions
                            );
                            continue;
                        }

                        // 4. Native Unqualified Call Fallback
                        let mut control_transferred = false;
                        let native_res = self.invoke_native(
                            call,
                            &mut ip,
                            &mut active_instructions,
                            &mut control_transferred,
                        );

                        if control_transferred {
                            continue;
                        }
                        self.stack.push(native_res);
                        ip += 1;
                        continue;
                    }
                }

                "CallNative" => {
                    if let Some(operand_json) = &instr.operand {
                        let call: TaskCallOperand = serde_json::from_value(operand_json.clone())
                            .expect("Invalid TaskCallOperand for CallNative");

                        // Tells us if a control transfer occurred (e.g. a coroutine method being called)
                        let mut control_transferred = false;
                        let result = self.invoke_native(call, &mut ip, &mut active_instructions, &mut control_transferred);

                        // If the 
                        if control_transferred {
                            continue;
                        }
                        self.stack.push(result);
                    } else {
                        panic!("[IP {:04}] Operand for CallNative is required.", ip);
                    }
                }


                /*
                 -------------------
                 ----- JUMPING -----
                 -------------------
                */


                "Jump" => {
                    if let Some(jump_to) = &instr.operand {
                        if let Some(jump_u) = jump_to.as_u64() {
                            ip = jump_u as usize;
                            continue; // We skip incremembting ip! This goes for the other jumps
                        }
                    }
                }

                // This checks for the condition before it to check if it can jump or not
                "JumpIfFalse" => {
                    if let Some(jump_to) = &instr.operand {
                        let condition = self.stack.pop().unwrap_or(Value::Nil);
                        
                        match condition {
                            // When false, DUH. Oh and nil I guess...
                            Value::Boolean(false) | Value::Nil => {
                                if let Some(jumpif_u) = jump_to.as_u64() {
                                    ip = jumpif_u as usize;
                                    continue;
                                }
                            }

                            // Gotta handle em possible situations too!
                            Value::Possible(possible) => {
                                let possible = possible.borrow();
                                if let Some(boolean) = possible.current && boolean == false && let Some(jumpif_u) = jump_to.as_u64() {
                                    ip = jumpif_u as usize;
                                    continue;
                                }
                            }

                            _ => () // You can't *break*, that'd break out the entire VM loop!
                        }
                    }
                }
                
                // Same here!
                "JumpIfPossible" => {
                    if let Some(jump_to) = &instr.operand {
                        let condition = self.stack.pop().unwrap_or(Value::Nil);

                        let should_jump = match &condition {
                            Value::Boolean(false) | Value::Nil => true,

                            Value::Possible(possible) => {
                                let possible = possible.borrow();
                                matches!(possible.current, Some(false))
                            }

                            _ => false, // Truthy booleans or unresolved Possibles do NOT jump
                        };

                        if should_jump {
                            if let Some(jumpif_u) = jump_to.as_u64() {
                                ip = jumpif_u as usize;
                                continue;
                            }
                        }

                        // Else just push the value back for reinspection by JumpIfFalse
                        self.stack.push(condition);
                    }
                }


                /*
                 ------------------
                 --- CANCELLING ---
                 ------------------
                */

                
                "Return" => {
                    // Return with no value
                    self.return_from_task(None, &mut ip, &mut active_instructions);

                    continue; // Continuing prevents the code from advancing the ip further out than it needs to
                }

                "ReturnValue" => {
                    // This one should have a value trailing it
                    let return_value = self.stack.pop();
                    self.return_from_task(return_value, &mut ip, &mut active_instructions);

                    continue;
                }

                "Yield" => {
                    // Get it like the returnable value. All Yields return a nil before them if no value is passed through them
                    let yielded_value = self.stack.pop();

                    // Now we check the active (innermost) call frame at the top of the call stack
                    let coroutine_opt = self.call_stack.last().and_then(|frame| frame.coroutine_instance.clone());

                    if let Some(coroutine) = coroutine_opt {
                        self.yield_task(yielded_value, &mut ip, &mut active_instructions, &coroutine);
                        continue;
                    } else {
                        panic!("[IP {:04}] Can only call Yield inside a running coroutine.", ip);
                    }
                }

                "Throw" => {
                    if let Some(operand) = &instr.operand {
                        // Try and get the message attached to the C# Exception sent
                        if let Some(err_message) = operand.get("Message") {
                            panic!("{}", err_message);
                        }
                        else {
                            // Panicking when the point of this code is to panic. How ironic!
                            panic!("No error message attached to exception when throwing! Could not find traceback.")
                        }
                    }
                }

                unknown => eprintln!("[IP {:04}] Unhandled OpCode: {}", ip, unknown),
            }

            ip += 1; // To next instruction
        }

        if print_completion {
            println!("Execution complete");
        }
    }

    /*
     ------------------------
     --- HELPER FUNCTIONS ---
     ------------------------
    */

    /// Loads all JSON files inside a given directory into the VM module map
    pub fn load_modules_from_dir(&mut self, dir_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            // When a json file, extract the stem name, then give it to the modules list
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let module_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid file name")?
                    .to_string();
                
                let json_data = std::fs::read_to_string(&path)?;

                let module: Bytecode = serde_json::from_str(&json_data)?;

                let modules: &mut HashMap<String, Bytecode> = Arc::make_mut(&mut self.modules);

                // Insert data in!
                modules.insert(module_name, module);
            }
        }
        Ok(())
    }

    /**
    Looks up the VM scopestack from the top for the first variable that matches the given name.
    If no variable is found, None is returned.

    ```
    # use crate::vm::VirtualMachine;
    # use crate::vm::VirtualMachine::lookup_variable;

    let vm = VirtualMachine::new();
    vm.scopes.push("VAR", VariableEntry::new(Value::String("A new variable!"), ..));

    let variable = lookup_variable("VAR")

    if let Some(unwrapped) = variable {
        println!("{}", unwrapped); // "A new variable!"
    }
    ```
     */
    fn lookup_variable(&self, name: &str) -> Option<&VariableEntry> {
        for frame in self.scopes.iter().rev() {
            if let Some(entry) = frame.get(name) {
                return Some(entry);
            }
        }

        None
    }

    /// Finds a direct child of `parent`: either an inline sub member,
    /// or a member of the enum that `parent` references (`Tool $ ToolTypes`).
    /// 
    /// ### Example
    /// ```
    /// # use crate::vm::VirtualMachine;
    /// # let VM = VirtualMachine::new(..);
    /// # let primary = EnumDefinition { .. };
    /// 
    /// // In context of a "Primary" enum being defined
    /// let secondary_enum = EnumMember {
    ///     ..
    ///     sub_members: vec![..]; // Includes a 'Tertiary' sub-member,
    ///     sub_enum_
    /// };
    /// 
    /// let tertiary_enum = VM.find_sub_member(&secondary_enum, "Tertiary").expect("Invalid example format");
    /// 
    /// assert_eq!("Tertiary", tertiary_enum.name);
    /// assert_eq!("Primary.Secondary.Tertiary", tertiary_enum.full_name());
    /// ```
    fn find_sub_member(&self, parent: &EnumMember, name: &str) -> Option<EnumMember> {
        // We just got sub members? Then simply search through the enum's sub-members
        // This looks simple, but I promise it wasn't easy to make
        if let Some(found) = parent.sub_members.iter().find(|m| m.name == name) {
            return Some(found.clone());
        }

        // Derefs the name, seeing if it is defined
        let reference = parent.sub_enum_reference.as_deref()?;

        // Defined in this module at runtime, otherwise search the loaded modules
        let target = self.enums.get(reference)
            .or_else(|| self.modules.values().find_map(|b| b.enums.get(reference)))?;

        target.enum_members.iter().find(|m| m.name == name).cloned()
    }

    pub fn eval_comparison(operation: &str, left: Value, right: Value) -> Value {
        match operation {
            "==" => left.evaluate_equal(&right),
            "!=" => left.evaluate_not_equal(&right),
            
            ">" | ">=" | "<" | "<=" => left.evaluate_relational(operation, &right),

            _ => panic!("Unknown comparison operator: {}", operation),
        }
    }

    pub fn negate_boollike(bool: &Value) -> Value {
        match bool {
            Value::Boolean(val) => { Value::Boolean(!val) },
            Value::Possible(possible) => match possible.borrow().current {
                Some(value) => Value::Boolean(!value),
                None => Value::new_possible(),
            },

            _ => panic!("Can only negate bool like values, got {}", bool)
        }
    }

    pub fn eval_indexing(index: Value, target: Value) -> Value {
        match target {
            List(list_vals) => {
                let list = list_vals.borrow();
                let items = &list.items;

                if let Value::Integer(i) = index {
                    // For indexing -1 (aka the last element)
                    if i == -1 {
                        // No empty lists!
                        if items.len() <= 0 {
                            panic!("Cannot index -1 on an empty list");
                        }

                        // Return safely
                        return items.last().unwrap_or(&Value::Nil).clone();
                    }

                    let item_count = items.len() as i64;

                    // With invalid indexing
                    if i < -1 || i >= item_count {
                        panic!("Indexing was out of reach for the indexed list");
                    }

                    // Get using usize (which Vectors only accept), and also clone to not attain ownership
                    items[i as usize].clone()
                } else {
                    panic!("Can only index a list using integers!");
                }
            }

            Value::String(str) => {
                if let Value::Integer(i) = index {
                    let char_vec: Vec<char> = str.chars().collect();
                    let chars_count = char_vec.len() as i64;

                    // -1 Indexing, aka the last element
                    if i == -1 {
                        // No No empty stringys!
                        if char_vec.is_empty() {
                            panic!("Cannot index -1 on an empty string");
                        }

                        return Value::String(char_vec.last().unwrap().to_string());
                    }

                    // When out of bounds
                    if i < 0 || i >= chars_count {
                        panic!("Indexing out of bounds: index {} on string of length {}", i, chars_count);
                    }

                    // Return as a string
                    Value::String(char_vec[i as usize].to_string())
                } else {
                    panic!("Can only index a string using integers!");
                }
            }

            Value::Dictionary(dictionary) => {
                let dictionary = dictionary.borrow();
                // We (again...) use zip to put the keys and values into nice pairs
                // Oh, and no type checking here btw
                for (key, val) in dictionary.keys.iter().zip(dictionary.values.iter()) {
                    if key == &index {
                        return val.clone(); // Return the exact matching value!
                    }
                }

                // If no key matched, panic with the index value displayed
                panic!("No key '{}' found in indexed dictionary", index);
            }

            _ => panic!("Invalid target type for indexing")
        }
    }

    /// Evaluates an index value on the target, then proceeds to update said index with the given value depending on the operation type given
    pub fn eval_update_indexing(index: Value, mut target: Value,
                                value: Value, operation: &str)
                                -> Result<Value, String> {
        match target {
            Value::List(ref list) => {
                let indx_i64 = match index {
                    Value::Integer(i) => i,
                    _ => return Err("Can only index a list using integers!".into()),
                };

                let mut list = list.borrow_mut();
                let len = list.items.len();
                
                // Convert index (allows handling -1 for last element)
                let actual_index = if indx_i64 == -1 {

                    if len == 0 {
                        return Err("Cannot index -1 on an empty list".into());
                    }

                    // Last element
                    len - 1
                }
                else if indx_i64 >= 0 && (indx_i64 as usize) < len {
                    indx_i64 as usize
                }
                else {
                    return Err(format!("Index {} out of bounds for list length {}", indx_i64, len));
                };

                // Now we just do the normal arithmetics...
                let current_val = &list.items[actual_index];

                let new_val = match operation {
                    "=" => value,
                    "+=" => Self::add_values(current_val, &value, None),
                    "-=" => Self::subtract_values(current_val, &value, None),
                    "*=" => Self::multiply_values(current_val, &value, None),
                    "/=" => Self::divide_values(current_val, &value, None),
                    "%=" => Self::modulo_values(current_val, &value, None),
                    _ => return Err(format!("Unknown operation: {}", operation)),
                };

                // Apply update directly, like right now. Like... RIGHT NOW
                list.items[actual_index] = new_val;
                // Drop the borrowed mut val
                drop(list);

                Ok(target)
            }

            Value::String(ref mut str) => {
                // Only allow assignment operator "=", nothin else
                if operation != "=" { 
                    return Err(format!("Cannot update a string with invalid operation: {}", operation)); 
                }

                // Only allow integer indexing, nothin else
                let i = match index {
                    Value::Integer(idx) => idx,

                    _ => return Err("Can only index a string using integers!".into()),
                };

                // Only allow a character value to replace the string index, nothin else
                let replacement_char = match value {
                    Value::String(ref s) if s.chars().count() == 1 => s.chars().next().unwrap(),

                    _ => return Err("Value assigned to string index must be a single-character String!".into()),
                };

                // Only collect UTF-8 characters... Well that's everything but, nothin else
                let mut chars: Vec<char> = str.chars().collect();
                let char_count = chars.len();

                // Only allow -1 to the length of the string indexing, nothin else
                let actual_index = if i == -1 {
                    // Because we handle -1 indexing first, the 'i >= 0' check is ignored :). Nothin else
                    if char_count == 0 {
                        return Err("Cannot index -1 on an empty string".into());
                    }

                    char_count - 1
                } else if i >= 0 && (i as usize) < char_count {

                    i as usize
                } else {

                    return Err(format!("Index {} out of bounds for string of character length {}", i, char_count));
                };

                // Update the character as it is mutable, nothin else
                chars[actual_index] = replacement_char;

                // Rust strings are immutable bytes, so we have to do all this just to finally
                // Reconstruct the string using the updated chars list. Nothin else.
                *str = chars.into_iter().collect();

                Ok(target)
            }

            Value::Dictionary(ref dictionary) => {
                let mut dictionary = dictionary.borrow_mut();
                // To shorten the process, we just go straight to finding the dictionary key instead of type matching
                // Find the value corresponding to the dictionary key
                let value_index = dictionary.keys.iter().position(|key| key == &index)
                    .ok_or_else(|| format!("No key '{}' found in indexed dictionary", index))?;

                let current_val = &dictionary.values[value_index];

                // Blah Blah Blah...
                let new_val = match operation {
                    "=" => value,
                    "+=" => Self::add_values(current_val, &value, None),
                    "-=" => Self::subtract_values(current_val, &value, None),
                    "*=" => Self::multiply_values(current_val, &value, None),
                    "/=" => Self::divide_values(current_val, &value, None),
                    "%=" => Self::modulo_values(current_val, &value, None),
                    _ => return Err(format!("Unknown operation: {}", operation)),
                };

                dictionary.values[value_index] = new_val;
                drop(dictionary);

                Ok(target)
            }

            _ => Err("Target is not indexable!".into()),
        }
    }

    /// Updates a variable entry using it's name, with the value given.
    /// This searches through all the scopes starting from the global scope.
    pub fn update_variable_value(&mut self , name: String, value: Value) {
        let mut changed_binding = None;
        for scope in self.scopes.iter_mut().rev() {
            let mutable_scope = Arc::make_mut(scope);

            if let Some(entry) = mutable_scope.get_mut(&name) {
                // Perform type checking compatibility
                if let Some(ensurable) = &entry.max_type {
                    MaxType::ensure_compatibility(ensurable, &value);
                }

                // Fixed variable check here!
                if entry.modifiers.contains(&String::from("fixed")) {
                    panic!("Cannot modify a fixed variable")
                }

                // We of course update the binding to whatever we are updating with
                *entry.value.borrow_mut() = value;
                changed_binding = Some(Rc::as_ptr(&entry.value) as usize);
                break;
            }
        }

        // Now we notify! Unless the binding was not defined, meaning no value was updated!
        let Some(binding_id) = changed_binding else {
            panic!("Variable of name '{name}' was not defined!");
        };
        self.notify_dependency(DependencyKey::Binding(binding_id));
    }

    /// Calls a lambda upon arguments (a vector of values).
    /// This runs the lambda on each argument individually
    pub fn call_lambda(&mut self, lambda: &RuntimeLambda, args: Vec<Value>) -> Value {
        let definition = &lambda.definition;

        // Argument Check!
        if args.len() as u64 != definition.arity {
            panic!("Lambda {} expects {} arguments, got {}", definition.id, definition.arity, args.len());
        }

        // This is the same logic you will see in the lambda section of "Call"
        let parameter_names = if definition.parameters.len() == args.len() {
            definition.parameters.clone()
        }
        else if definition.parameters.is_empty()
            && definition.arity == 1
        {
            definition.chunk.instructions.iter().find_map(|instruction| {
                (instruction.op_code == "Load")
                    .then(|| instruction.operand.as_ref().and_then(serde_json::Value::as_str))
                    .flatten()
                    .filter(|name| !lambda.captures.contains_key(*name))
                    .map(str::to_owned)
            })
            .into_iter().collect()
        }
        else {
            panic!("Lambda {} must provide parameter names for its arity", definition.id);
        };

        if parameter_names.len() != args.len() {
            panic!("Could not determine parameter names for lambda {}", definition.id);
        }

        // Allocate a hashmap for all the params
        let mut parameters = HashMap::with_capacity(args.len());
        for (name, value) in parameter_names.into_iter().zip(args) {
            parameters.insert(
                name,
                VariableEntry::new(
                    value,
                    vec![],
                    None,
                    None
                )
            );
        }

        // Get the instructions to run, along with the scope depth and return ip
        let instructions = Arc::clone(&definition.chunk.instructions);
        let return_ip = instructions.len(); // This is valid as when the instructions end, it will return to where the instructions first started from
        let scope_depth = self.scopes.len();

        // Create a new callframe...
        self.call_stack.push(CallFrame::new(
            None,
            None,
            return_ip,
            Arc::clone(&instructions),
            true,
            scope_depth,
            self.module_name.clone()
        ));

        // Push into the scopes the captured vars and params
        self.scopes.push(Arc::clone(&lambda.captures));
        self.scopes.push(Arc::new(parameters));

        // Create a new bytecode chunk we can run
        let nested_bytecode = Bytecode {
            instructions: Arc::clone(&instructions),
            tasks: Arc::clone(&self.task_definitions),
            coroutines: Arc::default(),
            enums: Arc::clone(&self.enums)
        };
        // Set prev instructions to the current active instructions!
        let previous_active_instructions = Arc::clone(&self.active_instructions);
        // Run it (btw no print completion so that the 'Execution complete' print statement doesn't run)
        self.run_chunk(self.module_name.clone(), nested_bytecode, false);
        // And now we reassign the previous active instructions back :)
        self.active_instructions = previous_active_instructions;

        // Pop the final Nil value returned
        self.stack.pop().unwrap_or(Value::Nil)
    }

    /// Attaches a possible condition upon the `PossibleHandle` via the `RuntimeLambda`.
    /// This also evaluates the possible immediately, so it may be instantaneously fixed to a value.
    pub fn attach_possible_condition(&mut self, possible: &PossibleHandle, condition: &RuntimeLambda) {
        {
            // Placing in this block lets 'signal' drop before running evaluate_possible
            let mut signal = possible.borrow_mut();
            signal.current = None;
            signal.condition = Some(condition.clone());
        }
        self.evaluate_possible(possible);
    }

    /// Inserts a dependency into the `Virtual Machine`'s `dependency_collector`, 
    /// allowing the dependency to be tracked.
    fn track_dependency(&mut self, dependency: DependencyKey) {
        if let Some(collector) = &self.dependency_collector {
            collector.borrow_mut().insert(dependency);
        }
    }

    /// Removes a dependency by first checking for the subscribers within `dependency_subscribers` that have been evaluated, 
    /// removing only those from the dependency in the process.
    fn remove_possible_subscriptions(&mut self, possible: &PossibleHandle, dependencies: &HashSet<DependencyKey>) {
        for dependency in dependencies {
            if let Some(subscribers) = self.dependency_subscribers.get_mut(dependency) {
                subscribers.retain(|subscriber| {
                    // For each subscriber, attempt to get their Rc reference, and then deduce if it is not the possible handle we are looking for
                    subscriber.upgrade().is_some_and(|candidate| !Rc::ptr_eq(&candidate, possible))
                });
            }
        }
    }

    /**
    Evaluate a possible value, setting it as true or false if it's assigned condition has been updated.
    If a possible has no condition attached, it is automatically removed from the evaluation list.
    The possible value is evaluated via calling the condition lambda it is attached with.
    Keep in mind that once evaluated, the possible still continues to track dependencies.

    ```
    let possibility: Value = Value::new_possible();
    let mut x: i64 = 3;
    let condition: Value = Value::Lambda(/* Condition inside is x > 5 */);

    Self::attach_possible_condition(possibility, condition);

    let eval: bool = possibility.borrow().current.unwrap();

    assert_eq!(eval, false);
    x = 6;
    assert_eq!(eval, true);

    // In order to stop evaluation, use our freeze method
    Self::freeze_possible(possibility);
    x = 3;
    // The possible no longer changes and reacts to the condition lambda
    assert_eq!(eval, true);
    ```
    */
    fn evaluate_possible(&mut self, possible: &PossibleHandle) {
        let possible_id = Rc::as_ptr(possible) as usize;
        if !self.evaluating_possible.insert(possible_id) {
            // Woah! Can't have possibles depending on themselves or eachother only, no? That'd be crazy
            panic!("Reactive Possible dependency cycle detected");
        }

        // Get the condition
        let condition = {
            let signal = possible.borrow();
            signal.condition.clone()
        };

        // No condition attached? Then we can't evaluate it!
        let Some(condition) = condition else {
            self.evaluating_possible.remove(&possible_id);
            return;
        };

        // This creates a collector, replaces the old collector with it, calls the lambda (to evaluate)
        let collector = Rc::new(RefCell::new(HashSet::new()));
        let previous_collector = self.dependency_collector.replace(Rc::clone(&collector));
        let result = self.call_lambda(&condition, vec![]);
        // Then resets the current dependency collector with the old one!
        self.dependency_collector = previous_collector;
        let dependencies = collector.borrow().clone();

        match result {
            // Got a bool? Now we can update!
            Value::Boolean(value) => {
                // Remove subscribers. Sorry, we like unsubscribing to Possible's channel here
                {
                    // Update the signal to now be finished
                    let mut signal = possible.borrow_mut();
                    signal.current = Some(value);
                }

                // Resubscribe here!
                let subscriber = Rc::downgrade(possible);
                for dependency in dependencies {
                    self.dependency_subscribers.entry(dependency).or_default().push(subscriber.clone());
                }

                // No longer being evaluated, and also notify dependency, for any other possibles attached to this one!
                self.evaluating_possible.remove(&possible_id);
                self.notify_dependency(DependencyKey::Possible(possible_id));
            }

            // If it's a pending possible
            value if value.is_pending_possible() => {
                {
                    let mut signal = possible.borrow_mut();
                    signal.current = None;
                    signal.dependencies = dependencies.clone();
                }
                let subscriber = Rc::downgrade(possible);

                for dependency in dependencies {
                    self.dependency_subscribers.entry(dependency).or_default().push(subscriber.clone());
                }

                self.evaluating_possible.remove(&possible_id);
            }

            // MUST be a boolean lambda
            _ => {
                self.evaluating_possible.remove(&possible_id);
                panic!("A Possible condition lambda must return Boolean or Possible");
            }
        }
    }

    /// Notifies within `dependency_subscribers` to only retain subscribers that still contain their signal, removing the frozen in the process.
    /// This then calls on each subscriber to be reevaluated.
    fn notify_dependency(&mut self, dependency: DependencyKey) {
        let subscribers = self.dependency_subscribers.get_mut(&dependency).map(|subscribers| {
            let mut active = Vec::new();
            subscribers.retain(|subscriber| {
                if let Some(possible) = subscriber.upgrade() {
                    active.push(possible);
                    true
                } else {
                    false
                }
            });
            active
        }).unwrap_or_default();

        for possible in subscribers {
            self.evaluate_possible(&possible);
        }
    }

    /// This simply applies iterable logic into `notify_dependency`
    pub fn notify_collection_changed(&mut self, collection: &Value) {
        match collection {
            Value::List(list) => self.notify_dependency(DependencyKey::List(Rc::as_ptr(list) as usize)),
            Value::Dictionary(dictionary) => self.notify_dependency(DependencyKey::Dictionary(Rc::as_ptr(dictionary) as usize)),

            _ => {}
        }
    }

    /// Freezes a possible handle, preventing the possible from reevaluating itself, and ensuring it's no longer subscribed to dependencies.
    pub fn freeze_possible(&mut self, possible: &PossibleHandle) {
        // Clone to not hold the borrow on 'possible'
        let old_dependencies = &possible.borrow().dependencies.clone();

        // Now we clear out the dependencies, subscription and condition
        self.remove_possible_subscriptions(possible, old_dependencies);
        {
            let mut possible = possible.borrow_mut();
            possible.condition = None;
            possible.dependencies.clear();
            // If possible is frozen whilst still possible, err!
            if let None = possible.current {
                panic!("Cannot freeze a possible value whilst it is still unevaluated!")
            }
        }
        let possible_id = Rc::as_ptr(possible) as usize;
        self.evaluating_possible.remove(&possible_id);
    }


    /*
        -----------------------------------
        --- HELPER ARITHMETIC FUNCTIONS ---
        -----------------------------------
    */

    /**
        Adds two Values together. Values in this context is from crate::vm::Value.
        This function also incorporates string concatenation as well.
        # Examples

        ```
        # use crate::vm::Value;
        # use crate::vm::VirtualMachine::add_values;

        let integer = Value::Integer(5);
        let float = Value::Float(6.34);

        let added = add_values(integer, float);

        assert_eq!(Value::Float(11.34), added);
        ```

        The ip (Instruction Pointer) is not necessary, but gives more information on the traceback to an error.
        The same logic goes for the other arithmetic helpers. However they do not incorporate string concatenation in any way.
    */
    pub fn add_values(left: &Value, right: &Value, ip: Option<Value>) -> Value {
        match (left, right) {
            // Integer and Float adding
            // With floats and integers, integers are promoted to floats
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a + b)
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a + b)
            }
            (Value::Float(a), Value::Integer(b)) => {
                Value::Float(a + *b as f64)
            }
            (Value::Integer(a), Value::Float(b)) => {
                Value::Float(*a as f64 + b)
            }

            // 2. String Concatenation: If either side is a String, convert both using Display
            (left_val, right_val) if matches!(left_val, Value::String(_)) || matches!(right_val, Value::String(_)) => {
                let result = format!("{}{}", left_val, right_val);
                Value::String(result)
            }

            (left_val, right_val) => {
                panic!(
                    "[IP {:04}] Cannot perform '+' operation between {:?} and {:?}",
                    ip.unwrap_or(Value::String(String::from("Unknown"))), left_val, right_val
                )
            }
        }
    }
    
    pub fn subtract_values(left: &Value, right: &Value, ip: Option<Value>) -> Value {
        match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a - b)
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a - b)
            }
            (Value::Float(a), Value::Integer(b)) => {
                Value::Float(a - *b as f64)
            }
            (Value::Integer(a), Value::Float(b)) => {
                Value::Float(*a as f64 - b)
            }

            (left_val, right_val) => {
                panic!(
                    "[IP {:04}] Cannot perform '-' operation between {:?} and {:?}",
                    ip.unwrap_or(Value::String(String::from("Unknown"))), left_val, right_val
                )
            }
        }
    }
    
    pub fn multiply_values(left: &Value, right: &Value, ip: Option<Value>) -> Value {
        match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a * b)
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a * b)
            }
            (Value::Float(a), Value::Integer(b)) => {
                Value::Float(a * *b as f64)
            }
            (Value::Integer(a), Value::Float(b)) => {
                Value::Float(*a as f64 * b)
            }

            (left_val, right_val) => {
                panic!(
                    "[IP {:04}] Cannot perform '*' operation between {:?} and {:?}",
                    ip.unwrap_or(Value::String(String::from("Unknown"))), left_val, right_val
                )
            }
        }
    }
    
    pub fn divide_values(left: &Value, right: &Value, ip: Option<Value>) -> Value {
        match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a / b)
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a / b)
            }
            (Value::Float(a), Value::Integer(b)) => {
                Value::Float(a / *b as f64)
            }
            (Value::Integer(a), Value::Float(b)) => {
                Value::Float(*a as f64 / b)
            }

            (left_val, right_val) => {
                panic!(
                    "[IP {:04}] Cannot perform '/' operation between {:?} and {:?}",
                    ip.unwrap_or(Value::String(String::from("Unknown"))), left_val, right_val
                )
            }
        }
    }
    
    pub fn modulo_values(left: &Value, right: &Value, ip: Option<Value>) -> Value {
        match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a % b)
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a % b)
            }
            (Value::Float(a), Value::Integer(b)) => {
                Value::Float(a % *b as f64)
            }
            (Value::Integer(a), Value::Float(b)) => {
                Value::Float(*a as f64 % b)
            }

            (left_val, right_val) => {
                panic!(
                    "[IP {:04}] Cannot perform '%' operation between {:?} and {:?}",
                    ip.unwrap_or(Value::String(String::from("Unknown"))), left_val, right_val
                )
            }
        }
    }


    /*
        --------------------------------
        ----- RETURNING / YIELDING -----
        --------------------------------
    */

    /// Returns from the task it is within, exitting all scopes the task has gone through in the process.
    /// This may also return a value to whatever it is assigning to
    pub fn return_from_task(&mut self, return_value: Option<Value>, ip: &mut usize, instructions: &mut Arc<Vec<Instruction>>) {
        if self.call_stack.len() <= 0 {
            panic!("Return can only be used in a valid task");
        }

        // Get the current call by popping, which essentially stops the task
        if let Some(frame) = self.call_stack.pop() {
            let returnable = return_value.unwrap_or(Value::Nil);
            
            // Ensure typing
            if let Some(coroutine) = &frame.coroutine_instance {
                if let Some(ensurable) = &coroutine.borrow().return_type {
                    MaxType::ensure_compatibility(ensurable, &returnable);
                }
            }
            // Same for a task
            else if let Some(task_info) = &frame.task_info {
                // If the typing is None, that means we don't expect any explicit typing!
                if let Some(ensurable) = &task_info.return_type {
                    MaxType::ensure_compatibility(ensurable, &returnable);
                } else if task_info.return_type != None {
                    panic!("Unknown error occured when matching the CallFrame {:#?}'s instance return type to the return value of {:#?}", frame, returnable);
                }
            }
            else if !frame.is_lambda {
                panic!("Unknown error occured when matching the CallFrame {:#?}'s instance return type to the return value of {:#?}", frame, returnable);
            }

            /*
                This while loop essentially exits out of ALL the scopes within the task.
                So even if the task was returning inside an if statement or yada yada...
                It still ends up going all the way back to the initial scope where the task was called!
            */
            while self.scopes.len() > frame.scope_depth {
                // Removes the very last scope
                self.scopes.pop();
            }

            // If it's a coroutine, simply reset!
            if let Some(coroutine) = &frame.coroutine_instance {
                let mut coroutine = coroutine.borrow_mut();
                coroutine.cancel();
                coroutine.instruction_point = coroutine.entry_point;
                coroutine.saved_scopes = None;
            }

            *ip = frame.return_ip;
            *instructions = frame.return_instructions;
            self.active_instructions = Arc::clone(instructions);
            if let Some(caller_bytecode) = self.modules.get(&frame.module_name) {
                self.task_definitions = Arc::clone(&caller_bytecode.tasks);
            }

            // Restore the module
            self.module_name = frame.module_name.clone();

            // Push 'nothing' onto the stack
            self.stack.push(returnable);

        }
    }

    /// Yields a coroutine, pausing the coroutine where it is and optionally bringing a value back as well
    pub fn yield_task(&mut self, yielded_value: Option<Value>, ip: &mut usize, instructions: &mut Arc<Vec<Instruction>>, coroutine: &CoroutineHandle) {
        if self.call_stack.len() <= 0 {
            panic!("Can only yield a value within a coroutine");
        }

        if let Some(frame) = self.call_stack.pop() {
            let mut coroutine_state = coroutine.borrow_mut();
            coroutine_state.instruction_point = (*ip + 1) as u64;
            coroutine_state.pause();

            if !self.scopes.is_empty() {
                // The scope where the coroutine can be reentered from (the beginning scope)
                let coroutine_reenterable = self.scopes.len().saturating_sub(frame.scope_depth);

                // The slice ranges for the saved scopes. It's actually basic maths when you look at it
                let start = frame.scope_depth;
                let end = start + coroutine_reenterable;

                // This just get's an iteration of the scopes we need
                let iterated = self.scopes[start..end]
                                    .iter().cloned();

                // And collects them into a nice vector
                let collected = iterated.collect();
                
                // Wrap in 'Some' as it is an Option
                coroutine_state.saved_scopes = Some(collected);
            }

            let yielded_unwrapped = yielded_value.unwrap_or(Value::Nil);

            drop(coroutine_state);

            if let Some(co_inst) = frame.coroutine_instance && let Some(ensurable) = &co_inst.borrow().return_type {
                MaxType::ensure_compatibility(ensurable, &yielded_unwrapped);
            }

            // Alike tasks logic
            while self.scopes.len() > frame.scope_depth {
                self.scopes.pop();
            }

            *ip = frame.return_ip;
            *instructions = frame.return_instructions;
            self.active_instructions = Arc::clone(instructions);

            self.stack.push(yielded_unwrapped);
        }
    }

    /// Attempts to invoke a native task from the native registries
    pub fn invoke_native(&mut self, call: TaskCallOperand, ip: &mut usize,
                          active_instr: &mut Arc<Vec<Instruction>>,
                          control_transferred: &mut bool)
                          -> Value {
        // Get all the args from popping off the stack

        let mut args = Vec::new();
        for _ in 0..call.argument_count {
            if let Some(val) = self.stack.pop() {
                args.push(val);
            }
        }
        args.reverse();

        let mut target_type = call.target;

        // If the targetted type was not resolved, instead look at the type of the first argument (i.e. it is always the 'self' value)
        // Oh, and btw, ONLY infer target type if this was explicitly invoked via dot syntax (member access)!
        if target_type.is_none() && call.is_member_access && args.len() > 0 && args[0] != Value::Nil {
            target_type = args[0].get_runtime_typename();
        }

        let target_str = target_type.as_deref();

        // Look up the native function using target_str (which will be None for globals)
        if let Some(native_fn) = self.native_registry.attempt_get_task(target_str, &call.name) {
            // And simply call!
            return (native_fn.function)(self, args);
        }

        // Creating binding to not drop value whilst in use
        let binding = target_type.unwrap_or(String::new());
        
        // Convert to Option<&str>
        let target_astr: Option<&str> = Some(binding.as_str());

        // Own the task before invoking it, since invocation mutably borrows the VM while the registry lookup otherwise holds an immutable borrow of it.
        if let Some(task) = self.native_registry.attempt_get_task(target_astr, &call.name).cloned() {
            // Copy data out of the registry before mutably borrowing the VM.
            // Check arg vs param count is done inside invoke() for us, and so is param type checking!

            let result = task.invoke(self, args);

            return result;
        }

        if let Some(prop) = self.native_registry.attempt_get_property(target_astr, &call.name).cloned() {
            // Check if it's a static module we are addressing
            let is_static = self.native_registry.is_static_module(target_astr.unwrap_or("").to_string());

            // Static properties expect 0 arguments; instance properties expect 1 ('self')
            let expected_args = if is_static { 0 } else { 1 };

            if call.argument_count != expected_args {
                panic!(
                    "Native property '{}' requires {} argument(s), but received {} arguments",
                    prop.name, expected_args, call.argument_count
                );
            }

            let return_type = prop.return_type.clone();

            // Pass in the self argument
            // For static modules, we pass in Nil to represent nothing (we need nothing for it!)
            let self_val = if is_static { Value::Nil } else { args[0].clone() };
            let result = prop.get(self, self_val);

            MaxType::ensure_compatibility(return_type.as_ref().unwrap_or(&MaxType::NoType), &result);

            return result;
        }

        if let Some(targettable) = target_astr {
            // We specifically target coroutines here!
            if targettable == "CoroutineType" {
                if call.argument_count != 1 {
                    panic!("Coroutine instance tasks only require a 'self' parameter, which is automatically inputted");
                }

                let coroutine = match &args[0] {
                    Value::Coroutine(co) => Rc::clone(co),

                    _ => panic!("Expected 'self' argument for a coroutine method call to be a coroutine instance")
                };

                self.execute_coroutine_task(&coroutine, &call.name, ip, active_instr, control_transferred);
                return Value::Nil;
            }
        }

        panic!("Native member '{}.{}' was not found.", target_astr.unwrap_or(""), call.name);
    }

    pub fn execute_coroutine_task(&mut self, coroutine: &CoroutineHandle, method: &str, ip: &mut usize,
                                    active_instr: &mut Arc<Vec<Instruction>>,
                                    control_transferred: &mut bool)
            {
        match method {
            "run" | "pause" | "cancel" | "terminate" => self.invoke_coroutine(coroutine, method, ip, active_instr, control_transferred),

            _ => panic!("Invalid coroutine method given, of name: {}", method)
        }
    }

    /// Invokes a user-defined task by pushing it onto the CallStack
    pub fn invoke_task(
        &mut self,
        task: TaskInfo,
        module_name: &str,
        argument_count: u64,
        ip: &mut usize,
        active_instructions: &mut Arc<Vec<Instruction>>,
    ) {
        if argument_count != task.parameters.len() as u64 {
            panic!(
                "Task '{}' expects {} arguments, but {} were provided",
                task.parameters.len(),
                task.parameters.len(),
                argument_count
            );
        }

        // Pop arguments from stack
        let mut arguments: Vec<Value> = Vec::with_capacity(argument_count as usize);
        for _ in 0..argument_count {
            if let Some(val) = self.stack.pop() {
                arguments.push(val);
            }
        }

        arguments.reverse();

        // Build parameters scope
        let mut call_scope: HashMap<String, VariableEntry> = HashMap::new();
        for index in 0..task.parameters.len() {
            let parameter = &task.parameters[index];
            let argument = &arguments[index];

            if let Some(ensurable) = &parameter.max_type {
                MaxType::ensure_compatibility(ensurable, argument);
            }

            call_scope.insert(
                parameter.name.clone(),
                VariableEntry::new(argument.clone(), vec![], None, None),
            );
        }

        // Get the bytecode of the modules to look for.
        // This is cleaner than having to pass in the bytecode as a param
        let target = self.modules.get(module_name).cloned()
            .unwrap_or_else(|| panic!("Module '{}' not found", module_name));

        // Record callframe pointing to caller's module/instructions
        self.call_stack.push(CallFrame {
            task_info: Some(task.clone()),
            coroutine_instance: None,
            return_ip: *ip + 1,
            return_instructions: Arc::clone(active_instructions),
            is_lambda: false,
            scope_depth: self.scopes.len(),
            module_name: self.module_name.clone(),
        });

        self.scopes.push(Arc::new(call_scope));

        // Update VM task definitions to match target module
        self.module_name = module_name.to_string();
        self.task_definitions = Arc::clone(&target.tasks);

        // Switch instruction stream to the target module's instructions
        *active_instructions = Arc::clone(&target.instructions);
        self.active_instructions = Arc::clone(active_instructions);

        // Jump instruction pointer to task entry point
        *ip = task.entry_point as usize;
    }

    /// Invokes a coroutine depending on the type of method given.
    /// Can be run, pause, cancel, or terminate
    pub fn invoke_coroutine(
        &mut self,
        coroutine: &CoroutineHandle,
        method: &str,
        ip: &mut usize,
        active_instr: &mut Arc<Vec<Instruction>>,
        control_transferred: &mut bool,
    ) {
        let state = coroutine.borrow().execution_state.clone();
        if state == ExecutionState::Dead {
            panic!("Cannot invoke a dead coroutine. Instead, try overriding the dead instance");
        }

        match method {
            "run" => {
                if state == ExecutionState::Running {
                    panic!("Cannot call run() on an already running coroutine");
                }

                // Push a new call telling us what the coroutine is and it's return ip/instructions
                self.call_stack.push(CallFrame {
                    task_info: None,
                    coroutine_instance: Some(Rc::clone(coroutine)),
                    return_ip: *ip + 1,
                    return_instructions: Arc::clone(active_instr),
                    is_lambda: false,
                    scope_depth: self.scopes.len(),
                    module_name: self.module_name.clone()
                });

                // Get the saved scopes if existing
                let saved_scopes = coroutine.borrow_mut().saved_scopes.take();

                if let Some(saved_scopes) = saved_scopes {
                    // Automatically go into the saved scopes that were already existing if needed!
                    self.scopes.extend(saved_scopes);
                } else {
                    self.scopes.push(Arc::new(HashMap::new()));
                }

                // And now we just restore to original place!
                let mut coroutine = coroutine.borrow_mut();
                coroutine.run();

                // Fetch the target module's instructions using coroutine_ref.module_name!
                let target_module_name = coroutine.module_name.clone();
                let target_bytecode = self.modules.get(&target_module_name)
                    .cloned()
                    .unwrap_or_else(|| panic!("Module '{}' not found for coroutine execution", target_module_name));

                // Reset back to norms!
                self.module_name = target_module_name;
                self.task_definitions = Arc::clone(&target_bytecode.tasks);

                *ip = coroutine.instruction_point as usize;
                *active_instr = Arc::clone(&target_bytecode.instructions);

                self.active_instructions = Arc::clone(active_instr);

                // Set to true, as it tells the call to not push anything further into the stack!
                *control_transferred = true;
            }

            "pause" => {
                if state == ExecutionState::Idle {
                    panic!("Cannot call pause() on an idle coroutine");
                }
                if state == ExecutionState::Paused {
                    panic!("Cannot call pause() on an already paused coroutine");
                }

                // If we are outside the coroutine, no need to exit it!
                if !self.is_current_coroutine(coroutine) {
                    coroutine.borrow_mut().pause();
                    return;
                }

                let frame = self.call_stack.pop().expect("active coroutine frame");
                let coroutine_reenterable = self.scopes.len().saturating_sub(frame.scope_depth);
                let saved_scopes = self.scopes[frame.scope_depth..frame.scope_depth + coroutine_reenterable].to_vec();
                
                {
                    // Get the coroutine. This is placed within a block as we are SHADOWING the coroutine (borrowing it here)
                    let mut coroutine = coroutine.borrow_mut();

                    // And pause it all! The instruction also will be put in the position right after the pausing!
                    coroutine.instruction_point = (*ip + 1) as u64;
                    coroutine.pause();
                    coroutine.saved_scopes = Some(saved_scopes);
                }

                // Restore to original position and push nil into the stack. Cause why not!
                self.restore_caller(frame, ip, active_instr);
                self.stack.push(Value::Nil);

                *control_transferred = true;
            }

            "cancel" => {
                if state == ExecutionState::Idle {
                    panic!("Cannot cancel an idle coroutine");
                }

                // Get the active index within the stack of where this frame is
                // NOTE: It's indexed via rposition (from the end), and searches for the coroutine to cancel
                let active_frame_index = self.call_stack.iter().rposition(|frame| {
                    frame.coroutine_instance.as_ref()
                        .is_some_and(|active| Rc::ptr_eq(active, coroutine))
                });

                // As usual. cancel the coroutine and reset it's ip and saved scopes
                {
                    let mut coroutine = coroutine.borrow_mut();

                    coroutine.cancel();
                    coroutine.instruction_point = coroutine.entry_point;
                    coroutine.saved_scopes = None;
                }

                // Now we get how far in we indexed if possible
                let Some(frame_index) = active_frame_index else {
                    return;
                };

                // Now we split the callstack off at where we found the cancellable coroutine
                let mut cancelled_frames = self.call_stack.split_off(frame_index);
                // And remove them!
                let frame = cancelled_frames.remove(0);

                // In each coroutine we cancel, also reset their properties
                for nested_frame in cancelled_frames {
                    if let Some(nested_coroutine) = nested_frame.coroutine_instance {
                        let mut nested_coroutine = nested_coroutine.borrow_mut();

                        nested_coroutine.cancel();
                        nested_coroutine.instruction_point = nested_coroutine.entry_point;
                        nested_coroutine.saved_scopes = None;
                    }
                }

                self.restore_caller(frame, ip, active_instr);

                // The cancelled coroutine's run call completes with nil.
                self.stack.push(Value::Nil);

                *control_transferred = true;
            }

            "terminate" => {
                // Given logic from cancel()
                let active_frame_index = self.call_stack.iter().rposition(|frame| {
                    frame.coroutine_instance.as_ref()
                        .is_some_and(|active| Rc::ptr_eq(active, coroutine))
                });

                // We KILL the coroutine
                {
                    let mut coroutine = coroutine.borrow_mut();

                    coroutine.terminate();
                    coroutine.saved_scopes = None;
                }

                let Some(frame_index) = active_frame_index else {
                    return;
                };

                let mut cancelled_frames = self.call_stack.split_off(frame_index);
                let frame = cancelled_frames.remove(0);

                // Kill all coroutines within the coroutine! A bit of a coroutine-cide, but...
                for nested_frame in cancelled_frames {
                    if let Some(nested_coroutine) = nested_frame.coroutine_instance {
                        let mut nested_coroutine = nested_coroutine.borrow_mut();

                        nested_coroutine.terminate();
                        nested_coroutine.instruction_point = nested_coroutine.entry_point;
                        nested_coroutine.saved_scopes = None;
                    }
                }
                
                self.restore_caller(frame, ip, active_instr);
                self.stack.push(Value::Nil); // Push nil through

                *control_transferred = true;
            }
            _ => panic!("Unknown method for invoking a coroutine found: {}", method),
        }
    }

    /// Helper function to check if the current call in the callstack is the coroutine given
    fn is_current_coroutine(&self, coroutine: &CoroutineHandle) -> bool {
        self.call_stack
            .last()
            .and_then(|frame| frame.coroutine_instance.as_ref())
            .is_some_and(|active| Rc::ptr_eq(active, coroutine))
    }

    /// Returns a callframe back to it's original call place, utilising it's return ip, and also resetting the active instructions to what is given.
    fn restore_caller(&mut self, frame: CallFrame, ip: &mut usize, instructions: &mut Arc<Vec<Instruction>>) {
        // Truncate very neatly just drops all the scopes the frame call went through, returning to the original scope
        self.scopes.truncate(frame.scope_depth);

        // Yada yada
        *ip = frame.return_ip;
        *instructions = frame.return_instructions;

        self.active_instructions = Arc::clone(instructions);
    }
}