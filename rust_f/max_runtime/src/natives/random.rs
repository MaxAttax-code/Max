use std::{cell::RefCell};

use crate::{natives::{NativeRegistry, ParamSpec}, vm::{VirtualMachine, Value}};
use max_bytecode::MaxType::{*};
use rand::{RngExt, SeedableRng, distr::{Distribution, weighted::WeightedIndex}, rngs::{StdRng, SysRng}, seq::SliceRandom};
use super::{NativeTask, NativeProperty, NativeTarget};


// |||||||||||||||||||||||||||||||||||||||| \\

// --- RNG THREADING --- \\

// This allows the RNG value to be shared across the random fns all the time
thread_local! {
    // A fresh seed directly from the OS. Starts random
    static RNG: RefCell<StdRng> = RefCell::new(StdRng::try_from_rng(&mut SysRng).unwrap())
}

/// Runs a closure with the shared generator
fn with_rng<T>(f: impl FnOnce(&mut StdRng) -> T) -> T {
    RNG.with(|rng| f(&mut rng.borrow_mut()))
}


// --------------------- \\


pub fn register_module(registry: &mut NativeRegistry) {
    let name = "random";


    // ----- REGISTERING TASKS ----- \\


    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "randInt",
            Some(IntegerType),
            vec![ParamSpec::required(IntegerType), ParamSpec::required(IntegerType)],
            self::random_rand_int
        ).doc("Gets a random integer between the minimum and maximum given")
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "random",
            Some(FloatType),
            vec![ParamSpec::required(FloatType), ParamSpec::required(FloatType)],
            self::random_rand_float
        ).doc("Gets a random float between the minimum and maximum given")
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "shuffle",
            Some(ListType(None)),
            vec![ParamSpec::required(ListType(None))],
            self::random_shuffle
        ).doc("Shuffles a given list and returns it. The complexity of a list of slices n is O(n)")
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "shuffleNow",
            None,
            vec![ParamSpec::required(ListType(None))],
            self::random_shuffle_now
        ).doc("Shuffles a given list in place")
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "weightedChoice",
            Some(NoType),
            vec![ParamSpec::required(ListType(None)), ParamSpec::variadic(IntegerType)],
            self::random_weighted_choice
        ).doc("Intakes a list of choices, and any following arguments are the weights attached to each individual item in the list. \
                The weights apply to the list items in order, and must be integers. Note that if less weights than the list length \
                were given, all remaining items are given a weight of 1 automaiclaly. This errs if more weights than list items were given.")
    );

    registry.register_task(
        NativeTarget::Module(name), NativeTask::new(
            "seed",
            None,
            vec![ParamSpec::required(IntegerType)],
            self::random_seed
        ).doc("Seeds the entire random module with the integer provided. Seeding this in any part of a project applies this to the rest of the code \
                that executes after this. Seeds allow the randomness to be defined for all of your modules.")
    );

    
    // ----- REGISTERING PROPERTIES ----- \\


    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "minInt",
            Some(IntegerType),
            IntegerType,
            self::random_min_int
        ).doc("Returns the minimum integer limit one can use")
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "maxInt",
            Some(IntegerType),
            IntegerType,
            self::random_max_int
        ).doc("Returns the maximum integer limit one can use")
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "minFloat",
            Some(FloatType),
            FloatType,
            self::random_min_float
        ).doc("Returns the minimum float limit one can use")
    );

    registry.register_property(
        NativeTarget::Module(name), NativeProperty::new(
            "maxFloat",
            Some(FloatType),
            FloatType,
            self::random_max_float
        ).doc("Returns the maximum float limit one can use")
    );
}


pub fn random_rand_int(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Integer(min), Value::Integer(max)] = args.as_slice() else {
        panic!("random.randInt expects a minimum and a maximum integer value");
    };

    if min > max { err_min_over_max(); }

    // INCLUSIVE
    let rng = with_rng(|r| r.random_range(*min..=*max));

    Value::Integer(rng)
}

pub fn random_rand_float(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let [Value::Float(min), Value::Float(max)] = args.as_slice() else {
        panic!("random.random expects a minimum and a maximum float value");
    };

    if min > max { err_min_over_max(); }

    // INCLUSIVE
    let rng = with_rng(|r| r.random_range(*min..=*max));

    Value::Float(rng)
}

pub fn random_shuffle(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let Value::List(list) = &args[0] else {
        panic!("random.shuffle expects a list")
    };
    let list = list.borrow();

    let mut new = list.items.to_vec();
    with_rng(|r| new.shuffle(r));
    
    Value::new_list(list.element_type.clone(), new)
}

/// Shuffles in place
pub fn random_shuffle_now(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let Value::List(list) = &args[0] else {
        panic!("random.shuffleNow expects a list")
    };
    let mut list = list.borrow_mut();
    with_rng(|r| list.items.shuffle(r));

    Value::Nil
}

pub fn random_weighted_choice(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let mut args_iter = args.into_iter();

    let list = match args_iter.next() {
        Some(v) => match v {
            Value::List(l) => l,
            _ => panic!("random.weightedChoice expects a list of choices first")
        },

        None => panic!("random.weightedChoice expects a list first")
    };
    let list = list.borrow();
    
    let mut weights: Vec<i64> = vec![];

    // As many times as there are list items
    for _ in 0..list.items.len() {
        let found = match args_iter.next() {
            Some(v) => match v {
                Value::Integer(i) => i,
                _ => panic!("random.weightedChoice expects integers for the weights")
            },

            // Always base 1
            None => 1
        };
        weights.push(found);
    }

    // Arguments are left? Nuh uh!
    if args_iter.len() != 0 {
        panic!("random.weightedChoice only accepts weight arguments up to the length of the list given")
    }

    let dist = WeightedIndex::new(&weights).unwrap_or_else(|c| panic!("{}", c.to_string()));
    // We finally choose!
    let chosen = list.items[with_rng(|r| dist.sample(r))].clone();

    chosen
}

pub fn random_seed(_vm: &mut VirtualMachine, args: Vec<Value>) -> Value {
    let Value::Integer(seed) = args[0] else {
        panic!("random.seed expects a non-negative integer seed");
    };

    if seed < 0 {
        panic!("random.seed expects a non-negative integer seed");
    }

    RNG.replace(StdRng::seed_from_u64(seed as u64));

    Value::Nil
}


/*
    ----------------------------
    ----- PROPERTY GETTERS -----
    ----------------------------
*/


pub fn random_min_int(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    Value::Integer(i64::MIN)
}

pub fn random_max_int(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    Value::Integer(i64::MAX)
}

pub fn random_min_float(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    Value::Float(f64::MIN)
}

pub fn random_max_float(_vm: &mut VirtualMachine, _s_arg: Value) -> Value {
    Value::Float(f64::MAX)
}


// |||||||||||||||||||||||||||||||||||||||| \\


// ----- HELPER FUNCTIONS ----- \\

pub fn err_min_over_max() { panic!("Gave a minimum value larger than the maximum value"); }