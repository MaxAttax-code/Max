use std::{
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use max_bytecode::Bytecode;

use crate::{
    ast::{Span, Statement, StatementKind},
    compiler::{CompileContext, Compiler, NativeNames, SymbolTable},
    error::MaxError,
    lexer::Lexer,
    parser::Parser,
};

/// The module the VM starts executing
pub const MAIN_MODULE: &str = "main";

// ---------------------------- //
// --------- ERRORS ----------- //
// ---------------------------- //

#[derive(Debug)]
pub enum ProjectError {
    Io { path: PathBuf, source: io::Error },
    /// A lexer, parser or compiler error, tagged with the file it came from
    InFile { path: PathBuf, error: MaxError },
    NoSourceFiles(PathBuf),
    DuplicateModule { module: String, first: PathBuf, second: PathBuf },
    /// The modules in the cycle, e.g. ["main", "util", "main"]
    Circular(Vec<String>),
    MissingMain,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io { path, source } => write!(f, "could not read {}: {source}", path.display()),
            ProjectError::InFile { path, error } => write!(f, "{}: {error}", path.display()),
            ProjectError::NoSourceFiles(dir) => write!(f, "no .max files found in {}", dir.display()),
            ProjectError::DuplicateModule { module, first, second } => write!(
                f,
                "'{module}' is the name of two modules: {} and {}",
                first.display(),
                second.display()
            ),
            ProjectError::Circular(chain) => write!(f, "circular dependency: {}", chain.join(" -> ")),
            ProjectError::MissingMain => write!(f, "could not find the main file to execute: '{MAIN_MODULE}.max'"),
        }
    }
}

impl std::error::Error for ProjectError {}

// ---------------------------- //
// ---------- TYPES ----------- //
// ---------------------------- //

/// A file's path and contents. Tests build these directly, so nothing here needs a real disk.
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

struct ParsedFile {
    module: String,
    path: PathBuf,
    statements: Vec<Statement>,
    /// The modules named by top-level `using` statements, without repeats
    dependencies: Vec<String>,
    /// Where each dependency was written, in the same order
    dependency_spans: Vec<Span>,
}

pub struct CompiledProgram {
    pub modules: HashMap<String, Bytecode>,
    /// The order the modules were compiled in, dependencies first
    pub order: Vec<String>,
}

// ---------------------------- //
// ------- ENTRY POINTS ------- //
// ---------------------------- //

/// Compiles every `.max` file under `source_dir`, found recursively.
/// Files are sorted by path, so lambda IDs and the compile order are the same on every machine.
pub fn compile_project(source_dir: &Path, natives: NativeNames) -> Result<CompiledProgram, ProjectError> {
    let mut paths = Vec::new();
    collect_max_files(source_dir, &mut paths)
        .map_err(|source| ProjectError::Io { path: source_dir.to_path_buf(), source })?;

    if paths.is_empty() {
        return Err(ProjectError::NoSourceFiles(source_dir.to_path_buf()));
    }
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = fs::read_to_string(&path)
            .map_err(|source| ProjectError::Io { path: path.clone(), source })?;
        sources.push(SourceFile { path, text });
    }

    compile_sources(sources, natives)
}

/// Compiles the given `SourceFiles` into `ParsedFiles`, then finally creating a `CompiledProgram` as a Result.
pub fn compile_sources(sources: Vec<SourceFile>, natives: NativeNames) -> Result<CompiledProgram, ProjectError> {
    let parsed = parse_sources(sources)?;

    if !parsed.iter().any(|file| file.module == MAIN_MODULE) {
        return Err(ProjectError::MissingMain);
    }

    let ordered = resolve_order(parsed, &natives)?;

    let mut ctx = CompileContext::new(natives);
    let mut modules = HashMap::new();
    let mut order = Vec::new();

    for file in &ordered {
        // A file's own symbols are visible while it compiles, same as the C# compiler
        ctx.symbols.insert(
            file.module.clone(),
            SymbolTable::from_statements(&file.module, &file.statements),
        );

        let bytecode = Compiler::new(&ctx, &file.dependencies)
            .compile(&file.statements)
            .map_err(|error| ProjectError::InFile { path: file.path.clone(), error })?;

        modules.insert(file.module.clone(), bytecode);
        order.push(file.module.clone());
    }

    Ok(CompiledProgram { modules, order })
}

/// Collects all .max extension files found within a path.
/// If the path leads to a directory, then search through the directory for .max files.
fn collect_max_files(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            collect_max_files(&path, found)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("max") {
            found.push(path);
        }
    }
    Ok(())
}

// ---------------------------- //
// --------- PARSING ---------- //
// ---------------------------- //

fn parse_sources(sources: Vec<SourceFile>) -> Result<Vec<ParsedFile>, ProjectError> {
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    let mut parsed = Vec::with_capacity(sources.len());

    for source in sources {
        // The module name is the file name without its extension
        let module = source
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();

        // `using` has no way to name a folder, so two files with one name could never be told apart
        if let Some(first) = seen.get(&module) {
            return Err(ProjectError::DuplicateModule {
                module,
                first: first.clone(),
                second: source.path,
            });
        }
        seen.insert(module.clone(), source.path.clone());

        let tokens = Lexer::new(&source.text)
            .tokenize()
            .map_err(|error| ProjectError::InFile { path: source.path.clone(), error })?;

        let statements = Parser::new(tokens)
            .parse()
            .map_err(|error| ProjectError::InFile { path: source.path.clone(), error })?;

        let mut dependencies: Vec<String> = Vec::new();
        // A vector of where these dependencies are located by Spans
        let mut dependency_spans = Vec::new();
        for statement in &statements {
            if let StatementKind::Using(using) = &statement.kind
                && !dependencies.contains(&using.module)
            {
                dependencies.push(using.module.clone());
                dependency_spans.push(using.module_span);
            }
        }

        parsed.push(ParsedFile { module, path: source.path, statements, dependencies, dependency_spans });
    }

    Ok(parsed)
}

// ---------------------------- //
// ---- DEPENDENCY ORDERING --- //
// ---------------------------- //

/// An Enum representing the state a module file is in during compiling file orders.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    Unvisited,
    /// On the current path. Meeting one of these again means a cycle
    Visiting,
    Visited,
}

struct Resolver<'a> {
    files: &'a [ParsedFile],
    index: HashMap<&'a str, usize>,
    natives: &'a NativeNames,
    state: Vec<Visit>,
    stack: Vec<usize>,
    order: Vec<usize>,
}

impl<'a> Resolver<'a> {
    fn visit(&mut self, i: usize) -> Result<(), ProjectError> {
        let files = self.files;

        match self.state[i] {
            Visit::Visited => return Ok(()),
            Visit::Visiting => {
                let start = self.stack.iter().position(|&s| s == i).unwrap_or(0);
                let mut chain: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|&s| files[s].module.clone())
                    .collect();
                chain.push(files[i].module.clone());
                return Err(ProjectError::Circular(chain));
            }
            Visit::Unvisited => {}
        }

        self.state[i] = Visit::Visiting;
        self.stack.push(i);

        for (d, dependency) in files[i].dependencies.iter().enumerate() {
            match self.index.get(dependency.as_str()) {
                Some(&j) => self.visit(j)?,

                // `using math;` is allowed, but there is no file to order
                None if self.natives.static_modules.contains(dependency) => {}

                None => {
                    let span = files[i].dependency_spans[d];
                    return Err(ProjectError::InFile {
                        path: files[i].path.clone(),
                        error: MaxError::Syntax {
                            message: format!("Could not find module '{dependency}'"),
                            line: span.line,
                            column: span.column,
                        },
                    });
                }
            }
        }

        self.stack.pop();
        self.state[i] = Visit::Visited;
        self.order.push(i);
        Ok(())
    }
}

/// Orders files so every file comes after the files it uses
fn resolve_order(files: Vec<ParsedFile>, natives: &NativeNames) -> Result<Vec<ParsedFile>, ProjectError> {
    let order = {
        let mut resolver = Resolver {
            files: &files,
            index: files.iter().enumerate().map(|(i, f)| (f.module.as_str(), i)).collect(),
            natives,
            state: vec![Visit::Unvisited; files.len()],
            stack: Vec::new(),
            order: Vec::new(),
        };

        for i in 0..files.len() {
            resolver.visit(i)?;
        }
        resolver.order
    };

    // Every index appears exactly once, so each slot is taken exactly once
    let mut slots: Vec<Option<ParsedFile>> = files.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .map(|i| slots[i].take().expect("each file is ordered once"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn natives() -> NativeNames {
        NativeNames { static_modules: HashSet::from([String::from("math")]), global_tasks: HashSet::new() }
    }

    fn file(name: &str, text: &str) -> SourceFile {
        SourceFile { path: PathBuf::from(name), text: text.to_string() }
    }

    #[test]
    fn dependencies_compile_before_the_files_that_use_them() {
        // `main` is listed first, but it needs `helper`
        let program = compile_sources(
            vec![
                file("src/main.max", "using helper;\nhelper.greet()"),
                file("src/helper.max", "define function greet => do say \"hi\" conclude"),
            ],
            natives(),
        )
        .unwrap();

        assert_eq!(program.order, vec!["helper", "main"]);
        assert!(program.modules.contains_key("main") && program.modules.contains_key("helper"));
    }

    #[test]
    fn circular_dependencies_report_the_cycle() {
        let result = compile_sources(
            vec![file("main.max", "using util;"), file("util.max", "using main;")],
            natives(),
        );

        match result {
            Err(ProjectError::Circular(chain)) => assert_eq!(chain, vec!["main", "util", "main"]),
            other => panic!("expected a cycle, got {:?}", other.err()),
        }
    }

    #[test]
    fn a_missing_module_points_at_its_using_statement() {
        let result = compile_sources(vec![file("main.max", "using nothing;")], natives());

        match result {
            Err(ProjectError::InFile { error: MaxError::Syntax { line, column, .. }, .. }) => {
                assert_eq!((line, column), (1, 7));
            }
            other => panic!("expected an error, got {:?}", other.err()),
        }
    }

    #[test]
    fn a_project_needs_a_main_file() {
        let result = compile_sources(vec![file("helper.max", "declare x = 1")], natives());
        assert!(matches!(result, Err(ProjectError::MissingMain)));
    }

    #[test]
    fn two_files_with_one_name_are_rejected() {
        let result = compile_sources(
            vec![file("a/util.max", ""), file("b/util.max", ""), file("main.max", "")],
            natives(),
        );
        assert!(matches!(result, Err(ProjectError::DuplicateModule { .. })));
    }

    #[test]
    fn using_a_native_module_is_allowed() {
        assert!(compile_sources(vec![file("main.max", "using math;\nsay math.sqrt(4)")], natives()).is_ok());
    }

    #[test]
    fn errors_name_the_file_they_came_from() {
        let result = compile_sources(vec![file("src/main.max", "declare x =")], natives());

        match result {
            Err(ProjectError::InFile { path, .. }) => assert_eq!(path, PathBuf::from("src/main.max")),
            other => panic!("expected an error, got {:?}", other.err()),
        }
    }
}