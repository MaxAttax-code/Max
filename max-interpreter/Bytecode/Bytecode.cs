using System.Text.Json;
using System.Text.Json.Serialization;

namespace Max
{
    public class Bytecode
    {
        /// <summary>
        /// The instructions the bytecode must execute
        /// </summary>
        public List<Instruction> Instructions { get; } = [];

        /// <summary>
        /// The tasks the bytecode holds
        /// </summary>
        public Dictionary<string, TaskInfo> Tasks { get; } = [];

        /// <summary>
        /// The coroutines the bytecode holds.
        /// Seperate from tasks
        /// </summary>
        public Dictionary<string, CoroutineInfo> Coroutines { get; } = [];

        /// <summary>
        /// The enums the bytecode holds
        /// </summary>
        public Dictionary<string, EnumeratorStatement> Enums { get; } = [];

        // Creating the options as a reusable instance is less performance-heavt
        private static readonly JsonSerializerOptions bytecodeExportOptions = new()
        {
            WriteIndented = true,
            Converters = { new JsonStringEnumConverter() }
        };

        /// <summary>
        /// A chunk of bytecode represented via instructions that are seperate from the Compiler
        /// </summary>
        public class BytecodeChunk
        {
            /// <summary>
            /// A flat list of instructions to execute sequentially
            /// </summary>
            public List<Instruction> Instructions { get; } = [];

            /// <summary>
            /// A pool for constants within the chunk to be held in
            /// </summary>
            public List<object> Constants { get; } = [];

            // Simply add to instructions via
            public void Emit(OpCode opCode, object? operand = null)
            {
                Instructions.Add(new Instruction(opCode, operand));
            }
        }

        /*
            ---------------------------
            --- TYPE REPRESENTATION ---
            ---------------------------
        */

        /// <summary>
        /// The base abstract representation of a type in serialization.
        /// Non-special types are categorised as PrimitiveType, otherwise are given their own type
        /// </summary>
        [JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
        [JsonDerivedType(typeof(PrimitiveTypeRepresentation), typeDiscriminator: "PrimitiveType")]
        [JsonDerivedType(typeof(ListTypeRepresentation), typeDiscriminator: "ListType")]
        [JsonDerivedType(typeof(DictionaryTypeRepresentation), typeDiscriminator: "DictionaryType")]
        public abstract class TypeRepresentation
        {
        }

        /// <summary>
        /// For primitive types. I.e. Types that have no recursion
        /// </summary>
        public class PrimitiveTypeRepresentation(string name) : TypeRepresentation
        {
            [JsonPropertyName("name")]
            public string Name { get; set; } = name;
        }

        // For lists, including their element type
        public class ListTypeRepresentation(TypeRepresentation? elementType) : TypeRepresentation
        {
            [JsonPropertyName("elementType")]
            public TypeRepresentation? ElementType { get; set; } = elementType;
        }

        // For dictionaries, including key and value types
        public class DictionaryTypeRepresentation(TypeRepresentation? keyType, TypeRepresentation? valueType) : TypeRepresentation
        {
            [JsonPropertyName("keyType")]
            public TypeRepresentation? KeyType { get; set; } = keyType;

            [JsonPropertyName("valueType")]
            public TypeRepresentation? ValueType { get; set; } = valueType;
        }

        public static TypeRepresentation? ToTypeRepresentation(MaxType? maxType)
        {
            if (maxType == null) return null;

            return maxType switch
            {
                // Handle parameterized container types
                ListType maxList => new ListTypeRepresentation(
                    ToTypeRepresentation(maxList.ElementType)
                ),
                
                DictionaryType maxDict => new DictionaryTypeRepresentation(
                    ToTypeRepresentation(maxDict.KeyType),
                    ToTypeRepresentation(maxDict.ValueType)
                ),
                
                // All primitive/non-recursive types
                _ => new PrimitiveTypeRepresentation(maxType.GetType().Name)
            };
        }

        /*
            ----------------------
            ----- CONVERSION -----
            ----------------------
        */

        /// <summary>
        /// A class that allows the conversion of Maxtype into JSON-readable
        /// </summary>
        public class MaxTypeJsonConverter : JsonConverter<MaxType>
        {
            /// <summary>
            /// Confirms if the Maxtype is valid, returning it in the process
            /// </summary>
            /// <param name="reader">The JSON reader that will read</param>
            /// <param name="typeToConvert">Using the General System.Type to access the MaxType</param>
            /// <param name="options">JsonSerializerOptions to implement with</param>
            /// <returns></returns>
            /// <exception cref="JsonException"></exception>
            public override MaxType? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
            {
                string? typeName = reader.GetString();
                // We just return null for nothing
                if (string.IsNullOrEmpty(typeName)) return null;

                // Map the string back to the MaxType instance using the TypeRegistry
                if (TypeRegistry.Max_TypeNames.TryGetValue(typeName, out MaxType? maxType))
                {
                    return maxType;
                }

                // When it is invalid
                throw new JsonException($"Unknown MaxType: '{typeName}'");
            }

            /// <summary>
            /// Writes the Maxtype into the string for the JSON to write into
            /// </summary>
            /// <param name="writer">The JSON reader to write it with</param>
            /// <param name="value">The MaxType. It's name is what will be converted into string</param>
            /// <param name="options">JsonSerializerOptions to implement with</param>
            public override void Write(Utf8JsonWriter writer, MaxType value, JsonSerializerOptions options)
            {
                // Convert MaxType to its type name
                writer.WriteStringValue(value.GetType().Name); 
            }
        }

        /// <summary>
        /// Exports the bytecode into JSON that the Rust analyzer can read
        /// </summary>
        /// <param name="bytecode">The Bytecode object to read</param>
        /// <param name="outputPath">Where to output the JSON</param>
        public static void ExportBytecode(Bytecode bytecode, string outputPath)
        {
            // Get directory too
            string? directory = Path.GetDirectoryName(outputPath);
            if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
            {
                Directory.CreateDirectory(directory);
            }
            
            string json = JsonSerializer.Serialize(bytecode, bytecodeExportOptions);
            File.WriteAllText(outputPath, json);
        }
    }
}
