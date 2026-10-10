// Under license to the reader by the author of the Max Language®
// Relaying these files and/or distributing or publishing them without the author's consent may incur a fine

using System.Text.Json.Serialization;

namespace Max {
    // We allow the JSON to convert it via our Convertor tool
    [JsonConverter(typeof(Bytecode.MaxTypeJsonConverter))]
    public abstract class MaxType
    {
    }

    public class NumberType : MaxType
    {
    }

    public class IntegerType : NumberType
    {
    }

    public class FloatType : NumberType
    {
    }

    public class StringType : MaxType
    {
    }

    public class BooleanType : MaxType
    {
    }

    public class ListType(MaxType? elementType = null) : MaxType
    {
        public MaxType? ElementType { get; } = elementType;
    }

    public class DictionaryType(MaxType? keyType = null, MaxType? valueType = null) : MaxType
    {
        public MaxType? KeyType { get; } = keyType;
        public MaxType? ValueType { get; } = valueType;
    }

    public class PossibleType : BooleanType
    {
    }

    /// <summary>
    /// A Type specifically to help distinguish coroutine variable instances from normal variables.
    /// </summary>
    public class CoroutineType : MaxType
    {
    }

    public static class TypeRegistry
    {
        public static readonly Dictionary<string, MaxType> Max_TypeNames = new()
        {
            { "int", new IntegerType() },
            { "float", new FloatType() },
            { "string" , new StringType() },
            { "boolean", new BooleanType() },
            { "list", new ListType() },
            { "dictionary", new DictionaryType() },
            { "coroutine", new CoroutineType() }
        };
    }
}