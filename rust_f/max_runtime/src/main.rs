// cargo run -p max_runtime

// ----- IMPORT CREATED CRATES -----\\

// Rather than compiling twice using 'mod', simply use them from the lib.rs itself
use max_runtime::{natives, vm::VirtualMachine};
use std::path::PathBuf;

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
        module_tasks: registry.module_task_keys(),
        module_properties: registry.module_property_keys(),
        global_tasks: registry.global_task_names(),
        transmutation_tasks: registry.transmutational_task_names()
    };

    let analysis = match compile_project(&project_dir.join("src"), &project_dir.join("library"), native_names) {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    for (path, diagnostics) in &analysis.diagnostics {
        for diagnostic in diagnostics {
            eprintln!("{}: {diagnostic}", path.display());
        }
    }

    let Some(program) = analysis.program else {
        // Just exit (DO NOT PANIC! Just keep calm and commit suicide!)
        eprintln!("Compilation failed; program was not loaded.");
        std::process::exit(1);
    };

    let mut vm = VirtualMachine::new();
    vm.load_program(program.modules);
    // We run this if a 'main' module file is found
    vm.run_main();
}