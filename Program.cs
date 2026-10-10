using Max;

CompiledProgram program = ProjectCompiler.CompileProject();

// Each module get's their own file, how neat!
foreach (CompiledFile compiledFile in program.CompiledFiles)
{
    // Get name without extension
    string moduleName = Path.GetFileNameWithoutExtension(compiledFile.Name);
    string filePath = Path.Combine("rust_f/bytecode_json", $"{moduleName}.json");

    // Export the bytecode for this specific module
    Bytecode.ExportBytecode(compiledFile.GetBytecode(), filePath);
}