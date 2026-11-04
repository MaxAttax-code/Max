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

/// Where a file came from. Library files are only compiled when something reaches them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A file in the project's own source directory
    User,
    /// A file in the built-in library directory
    Library,
}

/// A file's path and contents. Tests build these directly, so nothing here needs a real disk.
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
    pub origin: Origin
}

struct ParsedFile {
    module: String,
    path: PathBuf,
    origin: Origin,
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

/// Compiles every `.max` file under `source_dir`, found recursively,
/// along with whichever modules from `library_dir` they use.
/// Files are sorted by path, so lambda IDs and the compile order are the same on every machine.
pub fn compile_project(source_dir: &Path, library_dir: &Path, natives: NativeNames) -> Result<CompiledProgram, ProjectError> {
    // The two directories are collected into separate lists, so each file keeps track of where it came from
    let mut user_paths = Vec::new();
    collect_max_files(source_dir, &mut user_paths)
        .map_err(|source| ProjectError::Io { path: source_dir.to_path_buf(), source })?;

    if user_paths.is_empty() {
        return Err(ProjectError::NoSourceFiles(source_dir.to_path_buf()));
    }

    // Libraries in a seperate vec
    let mut library_paths = Vec::new();
    collect_max_files(library_dir, &mut library_paths)
        .map_err(|source| ProjectError::Io { path: library_dir.to_path_buf(), source })?;

    // All library paths are tagged with `Origin::Library` and users are tagged with `Origin::User`
    let mut tagged: Vec<(PathBuf, Origin)> = user_paths
        .into_iter()
        .map(|path| (path, Origin::User))
        .chain(library_paths.into_iter().map(|path| (path, Origin::Library)))
        .collect();
    // And sorted by paths
    tagged.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sources = Vec::with_capacity(tagged.len());
    for (path, origin) in tagged {
        let text = fs::read_to_string(&path)
            .map_err(|source| ProjectError::Io { path: path.clone(), source })?;
        sources.push(SourceFile { path, text, origin });
    }

    compile_sources(sources, natives)
}

/// Compiles the given `SourceFiles` into `ParsedFiles`, then finally creating a `CompiledProgram` as a Result.
pub fn compile_sources(sources: Vec<SourceFile>, natives: NativeNames) -> Result<CompiledProgram, ProjectError> {
    let parsed = parse_sources(sources)?;

    if !parsed.iter().any(|file| file.module == MAIN_MODULE && file.origin == Origin::User) {
        return Err(ProjectError::MissingMain);
    }

    let ordered = resolve_order(parsed)?;

    let mut ctx = CompileContext::new(natives);
    let mut modules = HashMap::new();
    let mut order = Vec::new();

    for file in &ordered {
        // A file's own symbols are visible while it compiles, same as the C# compiler
        ctx.symbols.insert(
            file.module.clone(),
            SymbolTable::from_statements(&file.module, &file.statements),
        );

        let bytecode = Compiler::new(&ctx, &file.module, &file.dependencies)
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
        let origin = source.origin;

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
            if let StatementKind::TaskDefinition(definition) = &statement.kind {
                if origin == Origin::User && definition.is_native {
                    return Err(ProjectError::InFile {
                        path: source.path,
                        error: MaxError::Logic("Cannot create a native task definition within a user-defined file".into())
                    });
                }
            }
        }

        parsed.push(ParsedFile { module, path: source.path, origin, statements, dependencies, dependency_spans });
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
                // This means the chain is already visiting this file
                // Leading to a circular loop
                let start = self.stack.iter().position(|&s| s == i).unwrap_or(0);
                let mut chain: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|&s| files[s].module.clone())
                    .collect();
                chain.push(files[i].module.clone());
                return Err(ProjectError::Circular(chain));
            }

            // It's alright
            Visit::Unvisited => {}
        }

        self.state[i] = Visit::Visiting;
        self.stack.push(i);

        for (d, dependency) in files[i].dependencies.iter().enumerate() {
            match self.index.get(dependency.as_str()) {
                Some(&j) => self.visit(j)?,

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

/// Orders files so every file comes after the files it uses.
/// Only user files are visited directly, so a library file survives only if something reaches it.
fn resolve_order(files: Vec<ParsedFile>) -> Result<Vec<ParsedFile>, ProjectError> {
    let order = {
        let mut resolver = Resolver {
            files: &files,
            index: files.iter().enumerate().map(|(i, f)| (f.module.as_str(), i)).collect(),
            state: vec![Visit::Unvisited; files.len()],
            stack: Vec::new(),
            order: Vec::new(),
        };

        for i in 0..files.len() {
            if files[i].origin == Origin::User {
                resolver.visit(i)?;
            }
        }
        resolver.order
    };

    // Each ordered index appears once. Library files nothing reached are never taken, and are dropped here
    let mut slots: Vec<Option<ParsedFile>> = files.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .map(|i| slots[i].take().expect("each file is ordered once"))
        .collect())
}