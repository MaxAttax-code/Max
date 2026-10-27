use std::collections::{HashMap, HashSet};

use crate::vm::{Value, VirtualMachine};
use max_bytecode::MaxType;

// ----- NATIVE MODS ----- \\

pub mod math;
pub mod iterables;

pub mod number;
pub mod string;
pub mod boolean;
pub mod list;
pub mod dictionary;

pub mod globals;

// ----------------------- \\

// --- NATIVE TASKS/PROPERTIES --- \\

pub type Nativefn = fn(&mut VirtualMachine, Vec<Value>) -> Value;

// Properties require only one param ('self')
pub type NativePropfn = fn(&mut VirtualMachine, Value) -> Value;

/// A structure that defines what kind of parameter is within a task.
/// Required: A required argument.
/// Optional: May be inputted, but not necessary to run the task.
/// Variadic: May have an infinite number of trailing arguments.
#[derive(Debug, Clone)]
pub enum ParamSpec {
    Required(Option<MaxType>),
    Optional(Option<MaxType>),
    Variadic(Option<MaxType>),
}

impl ParamSpec {
    /// Creates a required Param Spec with typing
    pub fn required(typing: MaxType) -> ParamSpec {
        ParamSpec::Required(Some(typing))
    }

    /// Creates a required Param Spec with no typing
    pub fn required_none() -> ParamSpec {
        ParamSpec::Required(None)
    }
    /// Creates an optional Param Spec with typing
    pub fn optional(typing: MaxType) -> ParamSpec {
        ParamSpec::Optional(Some(typing))
    }

    // /// Creates an optional Param Spec with no typing
    // pub fn optional_none() -> ParamSpec {
    //     ParamSpec::Optional(None)
    // }

    // /// Creates a variadic Param Spec with typing
    // pub fn variadic(typing: MaxType) -> ParamSpec {
    //     ParamSpec::Variadic(Some(typing))
    // }

    /// Creates a variadic Param Spec with no typing
    pub fn variadic_none() -> ParamSpec {
        ParamSpec::Variadic(None)
    }
}

/// A structure for a Native Task
#[derive(Debug, Clone)]
pub struct NativeTask {
    pub name: String,
    pub return_type: Option<MaxType>,
    pub parameter_types: Vec<ParamSpec>,
    pub function: Nativefn
}

impl NativeTask {
    pub fn new(
        name: &str,
        return_type: Option<MaxType>,
        parameter_types: Vec<ParamSpec>,
        function: self::Nativefn,
    ) -> Self {
        // Enforce that Variadic can ONLY be the last parameter spec
        // We do this NOW so that we don't have to check everytime the task is called!
        if let Some(variadic_pos) = parameter_types.iter().position(|p| matches!(p, ParamSpec::Variadic(_))) {
            if variadic_pos != parameter_types.len() - 1 {
                panic!(
                    "Native task '{}' invalid signature: Variadic parameter found at position {}, but must be the final parameter or only variadic parameter.",
                    name, variadic_pos
                );
            }
        }

        Self {
            name: name.to_string(),
            return_type,
            parameter_types,
            function,
        }
    }

    /// Invokes the function with type & parameter count validation
    pub fn invoke(&self, vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
        // Ensure argument vs parameter count here
        if let Err(error) = self.validate_arity(args.len()) {
            panic!("Invalid argument count for task, recieved error: {}", error)
        }

        // An index to keep track of which parameter we are on
        let mut param_idx = 0;

        // Through each argument
        for (arg_idx, arg) in args.iter().enumerate() {
            // Find which parameter spec applies to the current argument index
            let spec = if param_idx < self.parameter_types.len() {
                &self.parameter_types[param_idx]
            }
            else if let Some(ParamSpec::Variadic(_)) = self.parameter_types.last() {
                // Keep in mind that Variadic Params MUST be the final param, and tasks can only have ONE Variadic param!
                // All trailing arguments beyond fixed params (Required or Optional) belong to the variadic spec
                self.parameter_types.last().unwrap()
            }
            else {
                panic!("Native task '{}' received too many arguments: expected at most {}, got {}",
                        self.name, self.parameter_types.len(), args.len());
            };

            // Check depending on what type of param it is
            match spec {
                ParamSpec::Required(typing) => {
                    NativeTask::check_type_of_arg(&self.name.to_string(), arg_idx, typing, arg);
                    param_idx += 1;
                }
                ParamSpec::Optional(typing) => {
                    NativeTask::check_type_of_arg(&self.name.to_string(), arg_idx, typing, arg);
                    param_idx += 1;
                }
                ParamSpec::Variadic(typing) => {
                    // Check the current argument against variadic type rules
                    NativeTask::check_type_of_arg(&self.name.to_string(), arg_idx, typing, arg);
                    // Do NOT increment the parameter index so trailing arguments keep matching this variadic spec (It's Type as well)
                }
            }
        }

        // Ensure all remaining required parameters were supplied
        while param_idx < self.parameter_types.len() {
            if let ParamSpec::Required(_) = &self.parameter_types[param_idx] {
                panic!(
                    "Native task '{}' missing required argument at parameter index {}",
                    self.name, param_idx
                );
            }
            param_idx += 1;
        }

        // Execute function. Bzzzzzzzt!
        let result = (self.function)(vm, args);

        // Return Type Validation if needed
        if let Some(expected_return) = &self.return_type {
            if !VirtualMachine::check_value_compatibility(expected_return, &result) {
                panic!(
                    "Native task '{}' returned incompatible type {:?}, expected {:?}",
                    self.name, result, expected_return
                );
            }
        }

        result
    }

    /// Returns (`min_args`, `max_args`). If `max_args` is `None`, the function is variadic.
    pub fn arity_range(&self) -> (usize, Option<usize>) {
        let mut min = 0;
        let mut max = Some(0);

        for param in &self.parameter_types {
            match param {
                ParamSpec::Required(_) => {
                    min += 1;
                    max = max.map(|m| m + 1);
                }
                ParamSpec::Optional(_) => {
                    max = max.map(|m| m + 1);
                }
                ParamSpec::Variadic(_) => {
                    max = None; // Infinite upper bound
                }
            }
        }
        (min, max)
    }

    /// Validates if the given number of arguments fits the tasks parameter count.
    /// Variadic parameters are also considered within arity, and allow infinite arguments if set as.
    pub fn validate_arity(&self, arg_count: usize) -> Result<(), String> {
        let (min, max) = self.arity_range();

        // Can't have less than min, ever
        if arg_count < min {
            return Err(format!(
                "Task '{}' expected at least {} argument(s), but got {}",
                self.name, min, arg_count
            ));
        }

        // If not variadic
        if let Some(maximum) = max {
            if arg_count > maximum {
                return Err(format!(
                    "Task '{}' expected at most {} argument(s), but got {}",
                    self.name, maximum, arg_count
                ));
            }
        }

        Ok(())
    }

    /// Attemps to check an argument's type with the expected type if possible.
    /// The `task_name` and `arg_idx` params are simply debugging helpers
    pub fn check_type_of_arg(task_name: &str, arg_idx: usize, expected: &Option<MaxType>, arg: &Value) {
        if let Some(expected_type) = expected {
            let arg_type = arg.get_type();
            if !expected_type.is_assignable_to(&arg_type) {
                panic!(
                    "Native task '{}' argument {} type mismatch: expected {:?}, got {:?}",
                    task_name, arg_idx, expected_type, arg
                );
            }
        }
    }
}

/// A structure for a Native Property. In order to get it, you must call it's getter fn
#[derive(Debug, Clone)]
pub struct NativeProperty {
    pub name: String,
    pub return_type: Option<MaxType>,
    pub self_argument: MaxType,
    pub getter: NativePropfn,
}

impl NativeProperty {
    pub fn new(
        name: &str,
        return_type: Option<MaxType>,
        self_argument: MaxType,
        getter: self::NativePropfn,
    ) -> Self {
        Self {
            name: name.to_string(),
            return_type,
            self_argument,
            getter,
        }
    }

    pub fn get(&self, vm: &mut VirtualMachine, target: Value) -> Value {
        if !VirtualMachine::check_value_compatibility(&self.self_argument, &target) {
            panic!("Property '{}' target mismatch: expected {:?}, got {:?}",
                    self.name, self.self_argument, target);
        }
        (self.getter)(vm, target)
    }
}

// ----------------------- \\

// --- NATIVE REGISTRY --- \\

/// An enum that refers what type of Target a Native Property/Task is
pub enum NativeTarget<'a> {
    Module(&'a str),
    Type(&'a str),
    Global,
}

pub struct NativeRegistry {
    native_functions: HashMap<String, NativeTask>,
    native_properties: HashMap<String, NativeProperty>,
    static_modules: HashSet<String>
}

impl NativeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            native_functions: HashMap::new(),
            native_properties: HashMap::new(),
            static_modules: HashSet::new()
        };

        math::register_module(&mut registry);
        iterables::register_module(&mut registry);

        number::register_module(&mut registry);
        string::register_module(&mut registry);
        boolean::register_module(&mut registry);
        list::register_module(&mut registry);
        dictionary::register_module(&mut registry);

        globals::register_module(&mut registry);

        registry
    }

    /// Registers a native function within the registry so it can be called
    pub fn register_task(&mut self, target: NativeTarget, func: NativeTask) {
        let target_str = match target {
            NativeTarget::Module(name) => {
                // Also insert the module name into our HashSet
                self.static_modules.insert(name.to_string());
                Some(name)
            }
            NativeTarget::Type(name) => Some(name),
            NativeTarget::Global => None,
        };

        self.native_functions.insert(Self::get_key(target_str, &func.name), func);
    }

    /// Registers a native property within the registry so it can be obtained. Properties are called with a getter function.
    pub fn register_property(&mut self, target: NativeTarget, getter: NativeProperty) {
        let target_str = match target {
            NativeTarget::Module(name) => {
                self.static_modules.insert(name.to_string());
                Some(name)
            }
            NativeTarget::Type(name) => Some(name),
            NativeTarget::Global => None,
        };

        self.native_properties.insert(Self::get_key(target_str, &getter.name), getter);
    }

    /// Attempts to get a task of the target and name.
    /// This returns the function responsible for the task.
    pub fn attempt_get_task(&self, target: Option<&str>, name: &str) -> Option<&NativeTask> {
        let f_name = Self::get_key(target, name);
        if self.native_functions.contains_key(&f_name) {
            let res: Option<&NativeTask> = self.native_functions.get(&f_name);
            return res;
        }

        None
    }

    /// Attemps to get a property of the target and name.
    /// This returns the getter function for the property
    pub fn attempt_get_property(&self, target: Option<&str>, name: &str) -> Option<&NativeProperty> {
        let f_name = Self::get_key(target, name);
        if self.native_properties.contains_key(&f_name) {
            let res: Option<&NativeProperty> = self.native_properties.get(&f_name);
            return res;
        }

        None
    }

    /// Gets the `static_modules_names` HashSet by cloning `static_modules`
    pub fn static_module_names(&self) -> HashSet<String> {
        self.static_modules.clone()
    }

    /// Gets the `global_task_names` HashSet by filtering all of `native_functions`,
    /// searching only for functions without a member access
    pub fn global_task_names(&self) -> HashSet<String> {
        self.native_functions
            .keys()
            .chain(self.native_properties.keys())
            .filter(|key| !key.contains('.'))
            .cloned()
            .collect()
    }

    /// Simply checks if a given string is a static module name
    pub fn is_static_module(&self, module: String) -> bool {
        self.static_modules.contains(&module)
    }

    /// Get the name key to input into the registry. Allows a target and name, or name alone.
    pub fn get_key(target: Option<&str>, name: &str) -> String {
        if target.is_none() {
            return name.to_string()
        }

        // Format as target.name
        format!("{}.{}", target.unwrap(), name)
    }
}