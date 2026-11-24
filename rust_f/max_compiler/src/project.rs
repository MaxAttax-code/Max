use std::{
    collections::{HashMap, HashSet}, fmt, fs, io, path::{Path, PathBuf},
};

use max_bytecode::Bytecode;

use crate::{
    ast::{Span, Statement, StatementKind}, compiler::{CompileContext, Compiler, NativeNames, SymbolTable}, error::{MaxDiagnostic, Severity}, lexer::Lexer, parser::Parser,
};

/// The module the VM starts executing
pub const MAIN_MODULE: &str = "main";

// ---------------------------- //
// --------- ERRORS ----------- //
// ---------------------------- //

#[derive(Debug)]
pub enum ProjectError {
    Io { path: PathBuf, source: io::Error },
    NoSourceFiles(PathBuf),
    DuplicateModule { module: String, first: PathBuf, second: PathBuf },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io { path, source } => write!(f, "could not read {}: {source}", path.display()),
            ProjectError::NoSourceFiles(dir) => write!(f, "no .max files found in {}", dir.display()),
            ProjectError::DuplicateModule { module, first, second } => write!(
                f,
                "'{module}' is the name of two modules: {} and {}",
                first.display(),
                second.display()
            ),
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
    /// Represents whether the Parsed file had any `MaxDiagnostic`s returned
    failed: bool,
    /// The modules named by top-level `using` statements, without repeats
    dependencies: Vec<String>,
    /// Where each dependency was written, in the same order
    dependency_spans: Vec<Span>,
}

impl ParsedFile {
    pub fn failed(module: String, path: PathBuf, origin: Origin) -> Self {
        ParsedFile { module, path, origin, statements: Vec::new(), failed: true, dependencies: Vec::new(), dependency_spans: Vec::new() }
    }
}

pub struct CompiledProgram {
    pub modules: HashMap<String, Bytecode>,
    /// The order the modules were compiled in, dependencies first
    pub order: Vec<String>,
}

/// Holds the PathBuf of each file that contains a Vector of `MaxDiagnostic`
pub type FileDiagnostics = HashMap<PathBuf, Vec<MaxDiagnostic>>;

/// Holds the final Analysis of a file.
/// A file only becomes a `CompiledProgram` when it can compile without errors.
pub struct Analysis {
    /// Only `Some` when no file has an ERROR-severity diagnostic
    pub program: Option<CompiledProgram>,
    pub diagnostics: FileDiagnostics
}

// ---------------------------- //
// ------- ENTRY POINTS ------- //
// ---------------------------- //

/// Compiles every `.max` file under `source_dir`, found recursively,
/// along with whichever modules from `library_dir` they use.
/// Files are sorted by path, so lambda IDs and the compile order are the same on every machine.
pub fn compile_project(source_dir: &Path, library_dir: &Path, natives: NativeNames) -> Result<Analysis, ProjectError> {
    // The two directories are collected into separate lists, so each file keeps track of where it came from
    let mut user_paths = Vec::new();

    collect_max_files(source_dir, &mut user_paths)
        .map_err(|source| ProjectError::Io {
            path: dunce::canonicalize(
                source_dir.to_path_buf()
            ).unwrap_or(PathBuf::default()),
            source
        })?;

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
pub fn compile_sources(sources: Vec<SourceFile>, natives: NativeNames) -> Result<Analysis, ProjectError> {
    let mut diags: FileDiagnostics = HashMap::new();
    let mut broken: HashSet<&str> = HashSet::new();
    let parsed = parse_sources(sources, &mut diags)?;
    
    // The missing 'main.max' error is handled runtime

    let ordered: Vec<ParsedFile> = resolve_order(parsed, &mut diags);

    let mut ctx = CompileContext::new(natives);
    let mut modules = HashMap::new();
    let mut order = Vec::new();

    for file in &ordered {
        // The check if any diags are errors is a fallback
        let own_errors = file.failed
            || diags.get(&file.path).is_some_and(|d| d.iter().any(|d| d.severity == Severity::Error));
        // If itself OR a dependency is broken, insert this in
        if own_errors || file.dependencies.iter().any(|d| broken.contains(d.as_str())) {
            broken.insert(file.module.as_str());
            // Everything else is skipped!
            continue;
        }

        // A file's own symbols are visible while it compiles, same as the C# compiler
        ctx.symbols.insert(
            file.module.clone(),
            SymbolTable::from_statements(&file.module, &file.statements, &file.dependencies, &ctx.symbols),
        );
        // The compiled output errors are inputted into diagnostics
        let output = Compiler::new(&ctx, &file.module, &file.dependencies).compile(&file.statements);
        diags.entry(file.path.clone()).or_default().extend(output.diagnostics);

        // If it's valid bytecode or not
        match output.bytecode {
            Some(bytecode) => { modules.insert(file.module.clone(), bytecode); order.push(file.module.clone()); }
            None => { broken.insert(file.module.as_str()); }
        }
    }

    let has_errors = diags.values().flatten().any(|d| d.severity == Severity::Error);

    Ok(Analysis {
        program: (!has_errors).then(|| CompiledProgram { modules, order }),
        diagnostics: diags,
    })
}

/// Compiles the project with the given overlay, collecting any files found as well as from the overlay.
/// This searches from the `source_dir` as well as the `library_dir`, and checks if any files from the path did not match overlay,
/// otherwise Erring.
/// The process of tagging files as User or Library is identical to `compile_project`
pub fn compile_project_with_overlay(
        source_dir: &Path,
        library_dir: &Path,
        overlay: &HashMap<PathBuf, String>,
        natives: NativeNames,
    ) -> Result<Analysis, ProjectError>
{
    // Normalise the paths
    let source_dir = normalise(source_dir);
    let overlay: HashMap<PathBuf, String> = overlay.iter()
        .map(|(p, t)| (normalise(p), t.clone()))
        .collect();

    let mut user_paths = Vec::new();
    collect_max_files(&source_dir.to_path_buf(), &mut user_paths)
        .map_err(|source| ProjectError::Io { path: source_dir.to_path_buf(), source })?;

    let mut library_paths = Vec::new();
    collect_max_files(library_dir, &mut library_paths)
        .map_err(|source| ProjectError::Io { path: library_dir.to_path_buf(), source })?;

    for p in overlay.keys() {
        if !user_paths.contains(p) && !library_paths.contains(p) {
            user_paths.push(p.clone());
        }
    }

    let mut tagged: Vec<(PathBuf, Origin)> = user_paths.into_iter()
        .map(|path| (path, Origin::User))
        .chain(library_paths.into_iter().map(|path| (path, Origin::Library)))
        .collect();
    tagged.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sources = Vec::new();
    for (path, origin) in tagged {
        let text = match overlay.get(&path) {
            Some(t) => t.clone(),

            None => fs::read_to_string(&path)
                .map_err(|source| ProjectError::Io { path: path.clone(), source })?,
        };
        sources.push(SourceFile { path, text, origin });
    }
    
    compile_sources(sources, natives)
}

/// Collects all .max extension files found within a path.
/// If the path leads to a directory, then search through the directory for .max files.
fn collect_max_files(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            collect_max_files(&path, found)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("max") {
            found.push(dunce::canonicalize(&path)?);
        }
    }
    Ok(())
}

// ---------------------------- //
// --------- PARSING ---------- //
// ---------------------------- //

fn parse_sources(sources: Vec<SourceFile>, diags: &mut FileDiagnostics) -> Result<Vec<ParsedFile>, ProjectError> {
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
            if first != &source.path {
                return Err(ProjectError::DuplicateModule {
                    module,
                    first: first.clone(),
                    second: source.path,
                });
            }
            continue;
        }
        seen.insert(module.clone(), source.path.clone());

        let tokens = match Lexer::new(&source.text).tokenise() {
            Ok(tokens) => tokens,
            Err(d) => {
                diags.entry(source.path.clone()).or_default().push(d);
                parsed.push(ParsedFile::failed(module, source.path, origin));
                continue;
            }
        };
        
        // Maps all statements into single collected Vec
        // as well as all Diagnostics
        let (statements, parser_diagnostics) = Parser::new(tokens).parse();
        diags.entry(source.path.clone()).or_default().extend(parser_diagnostics);
        parsed.push(ParsedFile::failed(module.clone(), source.path.clone(), origin));

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
                    diags.entry(source.path.clone()).or_default().push(MaxDiagnostic::new(
                        Severity::Error, "user-native-task",
                        "Cannot create a native task definition within a user-defined file".to_owned(),
                        statement.span));
                }
            }
        }

        parsed.push(ParsedFile { module, path: source.path, origin, failed: false, statements, dependencies, dependency_spans });
    }

    Ok(parsed)
}

/// Normalise a path by using [`dunce::canonicalize`] on a valid path.
/// If that fails, then attempt to canonicalise with the parent of the given path, joining them if
/// the parent path and name are valid, else finally fallback to the path as a [`PathBuf`] with `to_path_buf`
pub fn normalise(path: &Path) -> PathBuf {
    if let Ok(p) = dunce::canonicalize(path) { return p; }
    match (path.parent().and_then(|d| dunce::canonicalize(d).ok()), path.file_name()) {
        (Some(dir), Some(name)) => dir.join(name),
        _ => path.to_path_buf(),
    }
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
    diags: &'a mut FileDiagnostics,
    index: HashMap<&'a str, usize>,
    state: Vec<Visit>,
    stack: Vec<usize>,
    order: Vec<usize>,
}

impl<'a> Resolver<'a> {
    fn visit(&mut self, i: usize) {
        // Visited: already done. Visiting: the caller reports it, so never re-enter either
        if self.state[i] != Visit::Unvisited { return; }

        // `files` is a Copy reference, so holding it doesn't borrow `self`
        // and we can still call `self.visit(..)` below
        let files = self.files;

        self.state[i] = Visit::Visiting;
        self.stack.push(i);

        for (d, dependency) in files[i].dependencies.iter().enumerate() {
            match self.index.get(dependency.as_str()).copied() {
                // This dependency is on the current path, so we've found a cycle
                Some(j) if self.state[j] == Visit::Visiting => self.report_cycle(j),

                Some(j) => self.visit(j),

                None => {
                    self.diags.entry(files[i].path.clone()).or_default().push(MaxDiagnostic::new(
                        Severity::Error,
                        "missing-module",
                        format!("Could not find module '{dependency}' needed as a dependency"),
                        files[i].dependency_spans[d],
                    ));
                }
            }
        }

        self.stack.pop();
        self.state[i] = Visit::Visited;
        self.order.push(i);   // still ordered, so recursion terminates
    }

    /// `back_to` is the file on the stack that something just tried to `using` again
    fn report_cycle(&mut self, back_to: usize) {
        let files = self.files;
        let start = self.stack.iter().position(|&s| s == back_to).unwrap_or(0);
        let cycle: Vec<usize> = self.stack[start..].to_vec();   // the files in the loop, in order

        let names: Vec<&str> = cycle.iter()
            .map(|&s| files[s].module.as_str())
            .chain(std::iter::once(files[back_to].module.as_str()))
            .collect();
        let message = format!("Circular dependency: {}", names.join(" -> "));

        // Every file in the cycle gets the error, on the `using` that points at the next file
        for (n, &file) in cycle.iter().enumerate() {
            let next = cycle.get(n + 1).copied().unwrap_or(back_to);
            let Some(d) = files[file].dependencies.iter().position(|m| *m == files[next].module)
                else { continue };

            self.diags.entry(files[file].path.clone()).or_default().push(MaxDiagnostic::new(
                Severity::Error,
                "circular-dependency",
                message.clone(),
                files[file].dependency_spans[d],
            ));
        }
    }
}

/// Orders files so every file comes after the files it uses.
/// Only user files are visited directly, so a library file survives only if something reaches it.
fn resolve_order(files: Vec<ParsedFile>, diags: &mut FileDiagnostics) -> Vec<ParsedFile> {
    let order = {
        let mut resolver = Resolver {
            files: &files,
            diags: diags,
            index: files.iter().enumerate().map(|(i, f)| (f.module.as_str(), i)).collect(),
            state: vec![Visit::Unvisited; files.len()],
            stack: Vec::new(),
            order: Vec::new(),
        };

        for i in 0..files.len() {
            if files[i].origin == Origin::User {
                resolver.visit(i);
            }
        }
        resolver.order
    };

    // Each ordered index appears once. Library files nothing reached are never taken, and are dropped here
    let mut slots: Vec<Option<ParsedFile>> = files.into_iter().map(Some).collect();
    order
        .into_iter()
        .map(|i| slots[i].take().expect("each file is ordered once"))
        .collect()
}