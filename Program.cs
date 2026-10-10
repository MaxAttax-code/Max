using Max;

CompiledProgram program = ProjectCompiler.CompileProject();

// Get all existing JSON files in the output directory
var existingJsonFiles =
    Directory.GetFiles("rust_f/bytecode_json", "*.json")
        .Select(Path.GetFileNameWithoutExtension)
        .ToHashSet();

// Get all module names that WILL be exported
var newModuleNames =
    program.CompiledFiles
        .Select(cf => Path.GetFileNameWithoutExtension(cf.Name))
        .ToHashSet();

// Delete JSON files that no longer correspond to any module
// This clears it up and prevents leftover code from somehow executing
foreach (var oldModule in existingJsonFiles)
{
    if (!newModuleNames.Contains(oldModule!))
    {
        string oldPath = Path.Combine("rust_f/bytecode_json", oldModule + ".json");
        File.Delete(oldPath);
    }
}

// Each module get's their own file, how neat!
foreach (CompiledFile compiledFile in program.CompiledFiles)
{
    // Get name without extension
    string moduleName = Path.GetFileNameWithoutExtension(compiledFile.Name);
    string filePath = Path.Combine("rust_f/bytecode_json", $"{moduleName}.json");

    // Export the bytecode for this specific module
    Bytecode.ExportBytecode(compiledFile.GetBytecode(), filePath);
}