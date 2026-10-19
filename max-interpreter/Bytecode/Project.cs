namespace Max
{
    public class ParsedFile(string name, List<IStatement> statements, List<string> dependencies)
    {
        public string Name { get; } = name;
        public IReadOnlyList<IStatement> Statements { get; } = statements;
        public IReadOnlyList<string> Dependencies { get; } = dependencies;
    }

    public class CompiledFile(string name, Bytecode bytecode)
    {
        public readonly string Name = name;
        private readonly Bytecode Bytecode = bytecode;

        public Bytecode GetBytecode()
        {
            return Bytecode;
        }
    }

    public class SymbolTable
    {
        public string ModuleName { get; set; } = string.Empty;
        public Dictionary<string, InfoRecord> Contents { get; } = [];

        public static SymbolTable FromParsedFile(ParsedFile file)
        {
            SymbolTable table = new() 
            { 
                ModuleName = Path.GetFileNameWithoutExtension(file.Name) 
            };

            foreach (var statement in file.Statements)
            {
                if (statement is TaskDefinition taskDef)
                {
                    // Differentiate based on the TaskType!
                    InfoRecord info = taskDef.TaskType switch
                    {
                        TaskType.Coroutine => new CoroutineInfo(taskDef.ReturnType),

                        _ => new TaskInfo(taskDef.Parameters, taskDef.ReturnType)
                    };

                    table.Contents.Add(taskDef.Name, info);
                }
                else if (statement is EnumeratorStatement enumStatement)
                {
                    InfoRecord info = new EnumInfo(enumStatement.EnumMembers);

                    table.Contents.Add(enumStatement.Identifier, info);
                }
            }

            return table;
        }
    }

    public class CompiledProgram(List<CompiledFile> files, CompiledFile mainFile)
    {
        public IReadOnlyList<CompiledFile> CompiledFiles { get; } = files;
        public CompiledFile MainFile { get; } = mainFile;
    }
    
    public class ProjectCompiler
    {
        public static CompiledProgram CompileProject()
        {
            /// Get source
            string sourceDirectory = "src";

            string[] files = Directory.GetFiles(
                sourceDirectory,
                "*.max",
                SearchOption.AllDirectories
            );

            // Create a native registry with JSON loaded into it
            string path = "rust_f/max_runtime/src/exported/export.json";
            NativeRegistry nativeRegistry = NativeRegistry.LoadFromJson(path);

            List<ParsedFile> parsedFiles = [];

            foreach (string source in files) {
                string name = Path.GetFileName(source);

                string sourceText = File.ReadAllText(source);

                Lexer lexer = new(sourceText);

                // Tokenise
                List<Token> tokens = lexer.Tokenize();

                Parser parser = new(tokens);

                // Parse
                List<IStatement> ast = parser.Parse();

                List<string> dependencies = [.. ast.OfType<UsingStatement>().Select(statement => statement.Module).Distinct()];

                ParsedFile parsedFile = new(
                    name,
                    ast,
                    dependencies
                );

                parsedFiles.Add(parsedFile);
            }

            // Resolve the dependencies in Topological order
            List<ParsedFile> orderedFiles = DependencyResolver.Resolve(parsedFiles);

            // The 'main.max' file
            CompiledFile? mainFile = null;
            // A list of compiled files, see the CompiledFile class for more
            List<CompiledFile> compiledFiles = [];
            // A global symbols table
            Dictionary<string, SymbolTable> globalSymbolTables = [];

            foreach (ParsedFile file in orderedFiles) {
                // First, get the name and symbols for the SymbolTable
                SymbolTable table = SymbolTable.FromParsedFile(file);
                string moduleName = Path.GetFileNameWithoutExtension(file.Name);

                globalSymbolTables[moduleName] = table;

                // Now we compile
                string name = file.Name;

                // Give the registry, along with global tools and imported dependencies
                Compiler compiler = new(nativeRegistry, globalSymbolTables, file.Dependencies);
                
                IReadOnlyList<IStatement> statements = file.Statements;

                compiler.Compile(statements);

                CompiledFile compiledFile = new(
                    name,
                    compiler.GetBytecode()
                );

                if (name == "main.max" && mainFile == null)
                {
                    mainFile = compiledFile;
                }

                compiledFiles.Add(compiledFile);
            }

            if (mainFile is null) throw new FileNotFoundException("Could not find the main file to execute: 'main.max'");

            // Create new program
            CompiledProgram newProgram = new(
                compiledFiles,
                mainFile
            );

            // Return a new Compiled Program!
            return newProgram;
        }
    }

    public class DependencyResolver
    {
        /// <summary>
        /// Tells us if a file is visited or not, or is being visited.
        /// Files currently being visitied cannot be visited again, otherwise detecting a circular dependency
        /// </summary>
        private enum VisitState
        {
            Unvisited,
            Visiting,
            Visited
        }

        /// <summary>
        /// Resolves a list of parsed files, topologically ordering them based on their dependencies
        /// </summary>
        /// <param name="files">The files to order</param>
        /// <returns>The new ordered files list</returns>
        public static List<ParsedFile> Resolve(List<ParsedFile> files)
        {
            // Produce a dictionary of file names to files
            Dictionary<string, ParsedFile> fileMap = files.ToDictionary(file => file.Name);

            Dictionary<string, VisitState> visitors = [];

            foreach (ParsedFile file in files)
            {
                visitors[file.Name] = VisitState.Unvisited;
            }

            List<ParsedFile> orderedFiles = [];

            // Begin with the top level files!
            foreach (ParsedFile file in files)
            {
                Visit(
                    file,
                    fileMap,
                    visitors,
                    orderedFiles
                );
            }

            return orderedFiles;
        }

        /// <summary>
        /// Visits a file, visiting any dependencies within the file, causing recursion.
        /// The file is then added to the orderedFiles list.
        /// </summary>
        /// <param name="file">The file to order first</param>
        /// <param name="fileMap">The current map of files</param>
        /// <param name="visitors">The current map of visited files</param>
        /// <param name="orderedFiles">The current list of ordered files</param>
        private static void Visit(
            ParsedFile file, Dictionary<string, ParsedFile> fileMap,
            Dictionary<string, VisitState> visitors, List<ParsedFile> orderedFiles)
        {
            if (visitors[file.Name] == VisitState.Visited)
            {
                // Already visiting? Bye!
                return;
            }

            if (visitors[file.Name] == VisitState.Visiting)
            {
                throw new MaxInfiniteLoopException($"Circular dependency detected involving {file.Name}");
            }

            visitors[file.Name] = VisitState.Visiting;

            // Now go through dependencies
            foreach (string dependency in file.Dependencies)
            {
                // Normalise
                string dependencyFileName = dependency + ".max";

                if (!fileMap.TryGetValue(dependencyFileName, out ParsedFile? dependencyFile))
                {
                    throw new MaxFileNotFoundException($"Could not find dependency '{dependency}' required by '{file.Name}'");
                }

                // Recursive visitting!
                Visit(
                    dependencyFile,
                    fileMap,
                    visitors,
                    orderedFiles
                );
            }
            
            visitors[file.Name] = VisitState.Visited;

            // Add in the neat order!
            orderedFiles.Add(file);
        }
    }
}