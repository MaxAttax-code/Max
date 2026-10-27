// cargo run -p max_runtime

// ----- IMPORT CREATED CRATES -----\\

mod natives;

pub mod vm;
use std::path::PathBuf;

use crate::vm::{VirtualMachine};
use max_compiler::{compiler::NativeNames, project::compile_project};

// ------ \\

/// Finds where the project directory is (where main.max must live)
fn find_project_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("src").join("main.max").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ------ \\

fn main() {
    let project_dir = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => find_project_dir().unwrap_or_else(|| {
            eprintln!("could not find src/main.max in this folder or any parent folder");
            std::process::exit(1);
        }),
    };

    let registry = natives::NativeRegistry::new();
    let native_names = NativeNames {
        static_modules: registry.static_module_names(),
        global_tasks: registry.global_task_names(),
    };

    let program = match compile_project(&project_dir.join("src"), native_names) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let mut vm = VirtualMachine::new();
    vm.load_program(program.modules);
    // We run this if a 'main' module file is found
    vm.run_main();
}