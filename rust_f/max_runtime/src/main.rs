// cargo run

// ----- IMPORT CREATED CRATES -----\\

pub mod bytecode;
mod natives;

pub mod vm;
use crate::vm::{VirtualMachine};

// ------ \\

use std::fs;
use std::path::Path;

// ------ \\

fn main() {
    let native_registry = natives::NativeRegistry::new();

    // First, let's Serialize all those native funcs and properties
    let registry_spec: natives::export::NativeRegistrySpec = natives::NativeRegistry::export_spec(&native_registry);

    // Serialize the spec into a formatted JSON string
    let json_data = match serde_json::to_string_pretty(&registry_spec) {
        Ok(json) => json,
        Err(err) => panic!("Failed to serialize native registry spec: {}", err),
    };

    // Ensure the output directory exists, just like your fath-
    let output_dir = Path::new("src/exported");
    if let Err(err) = fs::create_dir_all(output_dir) {
        panic!("Failed to create directory {:?}: {}", output_dir, err);
    }

    // Write the JSON string to the target file
    let file_path = output_dir.join("export.json");
    if let Err(err) = fs::write(&file_path, json_data) {
        panic!("Failed to write export.json at {:?}: {}", file_path, err);
    }

    // VM executes bytecode
    let mut vm = VirtualMachine::new();

    // We give all the module JSON files into the VM!
    vm.load_modules_from_dir("../bytecode_json").expect("Error occured whilst attempting to load all modules from bytecode_json");

    // We run this if a 'main.json' file is found
    vm.run_main();
}