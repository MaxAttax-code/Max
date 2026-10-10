namespace Max
{
    public class TaskDefinition(TaskType taskType, string name, List<Parameter> parameters, MaxType? returnType, List<IStatement> body) : IStatement
    {
        public TaskType TaskType { get; } = taskType;
        public string Name { get; } = name;
        public List<Parameter> Parameters { get; } = parameters;
        public MaxType? ReturnType { get; } = returnType;
        public List<IStatement> Body { get; } = body;

        public override string ToString()
        {
            return $"Task Definition of Type {TaskType},"
                 + $"Name of {Name}"
                 + $"Parameters of {Parameters}"
                 + $"Body of {Body}";
        }
    }

    // An enum for the state a coroutine is in
    public enum ExecutionState
    {
        Idle,
        Running,
        Paused,
        Dead
    }

    // For native coroutine methods
    public class CoroutineInstance(int entryPoint, MaxType? returnType)
    {
        public ExecutionState State { get; private set; } = ExecutionState.Idle; // Always start off as idle!

        public MaxType? ReturnType { get; } = returnType;
        
        /// <summary>
        /// Where the coroutine currently is in it's placement.
        /// Starts as the entry point automatically.
        /// </summary>
        public int InstructionPoint { get; set; } = entryPoint;

        /// <summary>
        /// Where the coroutine begins
        /// </summary>
        public int EntryPoint { get; } = entryPoint;

        /// <summary>
        /// The saved scopes that the coroutine has travelled through.
        /// Used for when pausing, where the coroutine accesses this to reenter the saved scopes.
        /// </summary>
        public List<Dictionary<string, Variable>>? SavedScopes { get; set; }

        public void Run() => State = ExecutionState.Running;

        public void Pause() => State = ExecutionState.Paused; // Indicates it is in a saved position 

        public void Cancel()
        {
            // Reset state, instruction pointer, and local variables back to default
            State = ExecutionState.Idle;
            InstructionPoint = EntryPoint;
            SavedScopes = null;
        }

        public void Terminate() => State = ExecutionState.Dead;
    }
}