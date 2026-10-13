using System.Text.Json.Serialization;

namespace Max
{
    public class Instruction(OpCode opCode, object? operand = null)
    {
        [JsonPropertyName("OpCode")]
        public OpCode OpCode { get; } = opCode;
        [JsonPropertyName("Operand")]
        public object? Operand { get; } = operand;
    }

    /// <summary>
    /// Stores a variable name and it's corresponding type and resort value if needed
    /// </summary>
    /// <param name="Name">The name of the variable</param>
    /// <param name="Type">The variable type, may be null</param>
    /// <param name="ResortValue">The resort value of the variable, if needed</param>
    public record StoreOperand(string Name, HashSet<string> Modifiers, Bytecode.TypeRepresentation? Type, object? ResortValue);

    /// <summary>
    /// A record of operations and their names
    /// </summary>
    /// <param name="Name">The name of the variable</param>
    /// <param name="Operation">The operation to update via</param>
    public record UpdateOperand(string Name, string Operation);

    /// <summary>
    /// A record specifically to update an index
    /// </summary>
    /// <param name="Operation">The operation to update via</param>
    /// <param name="TargetName">The target for the operation</param>
    public record UpdateIndexOperand(string Operation, string? TargetName);

    /// <summary>
    /// A record to hold the target and member of a member access operand in one!
    /// </summary>
    /// <param name="target"></param>
    /// <param name="member"></param>
    public record MemberAccessOperand(string Target, string Member);

    /// <summary>
    /// A record of a foreach item; using the collection and index slot, and the variable name
    /// </summary>
    /// <param name="CollectionSlot">A temporary slot for the collection</param>
    /// <param name="IndexSlot">A temporary slot for the current/chosen item index</param>
    /// <param name="Declarations">The variable names of the declaration(s) to be done</param>
    /// <param name="VariableType">The optional declared type of the iteration variable</param>
    public record ForeachOperand(
        int CollectionSlot,
        int IndexSlot,
        List<string> Declarations,
        MaxType? VariableType
    );

    /// <summary>
    /// A record that collects up the types of records that simply give info on the location of their object-to-be-found.
    /// </summary>
    public abstract record InfoRecord();

    /// <summary>
    /// A record of Task Info, containing it's necessary data on a task
    /// </summary>
    /// <param name="Parameters">The parameters the task accepts</param>
    /// <param name="ReturnType">The return type of the task, regarded as null when none given</param>
    /// <param name="EntryPoint">Where the task begins executing</param>
    public record TaskInfo(
        List<Parameter> Parameters,
        MaxType? ReturnType,
        int EntryPoint = -1
    ) : InfoRecord;

    public record CoroutineInfo(MaxType? ReturnType, int EntryPoint = -1) : InfoRecord;

    public record TaskReference(string? Target, string Name);

    /// <summary>
    /// A record that supplies a Task Call with the name and arguments
    /// </summary>
    /// <param name="Target">The optional target/library name</param>
    /// <param name="Name">The local name of the task to call</param>
    /// <param name="ArgumentCount">The number of arguments passed in</param>
    /// <param name="IsMemberAccess">If the task call was member accessed</param>
    public record TaskCallOperand(string? Target, string Name, int ArgumentCount, bool IsMemberAccess = false);

    /// <summary>
    /// A record that allows a Lambda Operation to function with it's given params and acts
    /// </summary>
    /// <param name="ID">An ID for which the Lambda is assigned to. The VM can access it via it's id.</param>
    /// <param name="Arity">The number of params the lambda intakes.</param>
    /// <param name="Parameters">The names of the parameters within the lambda</param>
    /// <param name="Captured">The list of captured variables within the params.</param>
    /// <param name="Chunk">The executable bytecode instructions for the lambda.</param>
    public record LambdaOperand(int ID, int Arity, List<string> Parameters, List<string> Captured, Bytecode.BytecodeChunk Chunk);
}