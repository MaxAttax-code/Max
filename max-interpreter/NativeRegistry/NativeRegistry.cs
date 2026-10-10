using System.Text.Json;

namespace Max
{
    /// <summary>
    /// A registry of all the native functions within Max
    /// </summary>
    public class NativeRegistry
    {
        private readonly HashSet<string> staticModules = [];
        private readonly Dictionary<string, NativeTaskSpec> globalTasks = [];
        private readonly Dictionary<(string module, string task), NativeTaskSpec> moduleTasks = [];

        // Options to read using snake_case
        private static readonly JsonSerializerOptions JsonOptions = new()
        {
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
        };

        /// <summary>
        /// Reads from the JSON file containing the Native Specs, and adds the given 
        /// json deserialized to the newly returned Registries private fields
        /// </summary>
        /// <param name="jsonPath"></param>
        /// <returns></returns>
        public static NativeRegistry LoadFromJson(string jsonPath)
        {
            var json = File.ReadAllText(jsonPath);
            var spec = JsonSerializer.Deserialize<NativeRegistrySpec>(json, JsonOptions) ?? throw new Exception("Attempted to Deserialize JSON file for NativeRegistry, recieved null");
            
            var registry = new NativeRegistry();
            
            foreach (var mod in spec.StaticModules)
            {
                registry.staticModules.Add(mod.Name);
                foreach (var task in mod.Tasks)
                {
                    registry.moduleTasks[(mod.Name, task.Name)] = task;
                }
            }

            foreach (var task in spec.GlobalTasks)
            {
                registry.globalTasks[task.Name] = task;
            }

            return registry;
        }

        public bool IsStaticModule(string name) => staticModules.Contains(name);
        
        public bool TryGetTask(string? module, string name, out NativeTaskSpec task)
        {
            if (module == null)
                return globalTasks.TryGetValue(name, out task!);
            
            return moduleTasks.TryGetValue((module, name), out task!);
        }
    }
}