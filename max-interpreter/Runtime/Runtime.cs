namespace Max
{
    #region Run-Time Classes

    /// <summary>
    /// Runtime reference of a possible keyword
    /// </summary>
    public class Possible : BoolLike
    {
        public string Value { get; } = "possible";
        public override string ToString()
        {
            return "Possible";
        }
    }

    public class Bool(bool value) : BoolLike
    {
        public bool Value { get; } = value;

        public override string ToString()
        {
            return Value.ToString();
        }
    }

    public class BoolLike
    {
    }

    public class Nil
    {
        public string Value { get; } = "nil";
        public override string ToString()
        {
            return "nil";
        }
    }

    /// <summary>
    /// A class that represemts a variable and all the elements it can hold.
    /// This is it's 'VariableEntry' correspondent within the Rust Bytecode.
    /// </summary>
    /// <param name="name">The variable name</param>
    /// <param name="value">The value assigned</param>
    /// <param name="modifiers">The declaration modifiers on this variable when declared. May be an empty set.</param>
    /// <param name="type">The type given, if so</param>
    /// <param name="resortValue">The resort value assigned, if so</param>
    public class Variable(string name, object value, ModifierSet modifiers, MaxType? type, object? resortValue)
    {
        public string Name { get; } = name;
        public object Value { get; private set; } = value;
        public ModifierSet Modifiers { get; } = modifiers;
        public MaxType? Type { get; } = type;
        public object? ResortValue { get; private set; } = resortValue;

        public void SetValue(object value)
        {
            Value = value;
        }

        public void SetResort(object resortValue)
        {
            // Cannot be set as nil
            if (resortValue is Nil)
            {
                throw new MaxVariableException("Cannot set nil as a resort value");
            }

            // Only able to resort value once (when null)
            ResortValue ??= resortValue;
        }
    }

    public class GlobalVariableInfo(MaxType? type, ModifierSet modifierSet)
    {
        public MaxType? Type { get; private set; } = type;
        public ModifierSet Modifiers { get; } = modifierSet;
    }

    /// <summary>
    /// The bare minimum for a Callable Task, whether native or defined
    /// </summary>
    public interface IMaxCallable
    {
        List<Parameter> Parameters { get; }
        MaxType? ReturnType { get; }
    }

    public class TaskRunnable(TaskType taskType, string name, List<Parameter> parameters, MaxType? returnType, List<IStatement> body) : IMaxCallable
    {
        public TaskType TaskType { get; } = taskType;
        public string Name { get; } = name;
        public List<Parameter> Parameters { get; } = parameters;
        public MaxType? ReturnType { get; } = returnType;
        public List<IStatement> Body { get; } = body;

        public override string ToString()
        {
            return $"Task Runnable of Type {TaskType},"
                 + $"Name of {Name}"
                 + $"Parameters of {Parameters}"
                 + $"Body of {Body}"
                 + $"Return type of {ReturnType}";
        }
    }

    #region Native Runtime

    // A delegate for the function a Max Native Task performs
    public delegate object? MaxNativeFunction(object?[] arguments);

    /// <summary>
    /// A Task that is native to Max
    /// </summary>
    /// <param name="name">The name of the task</param>
    /// <param name="returnType">It's return type</param>
    /// <param name="parameterTypes">It's Parameter types, as Read Only</param>
    /// <param name="function">The function that will be performed</param>
    public class NativeTask(string name, MaxType? returnType,
                      IReadOnlyList<MaxType?> parameterTypes, MaxNativeFunction function)
    {
        public string Name { get; } = name;
        /// <summary>
        /// A null return type means it can return any type, not that it doesn't return a type at all
        /// </summary>
        public MaxType? ReturnType { get; } = returnType;
        /// <summary>
        /// Please note that null translates to 'any type' essentially for parameters
        /// </summary>
        public IReadOnlyList<MaxType?> ParameterTypes { get; } = parameterTypes;
        public MaxNativeFunction Function { get; } = function;

        public override string ToString()
        {
            return $"Native Task of name '{Name}', "
                 + $"Return Type of {ReturnType}, "
                 + $"Parameter Types: [{string.Join(", ", ParameterTypes)}], "
                 + $"Function given of {Function.Method}";
        }
    }

    public class NativeProperty(string name, MaxType? returnType,
                                MaxType selfArgument, MaxNativeFunction getter)
    {
        // This is a class just for getting a native property
        
        public string Name { get; } = name;
        /// <summary>
        /// Can be null, meaning it can return any kind of type
        /// </summary>
        public MaxType? ReturnType { get; } = returnType;
        public MaxType SelfArgument { get; } = selfArgument;
        public MaxNativeFunction Function { get; } = getter;

        public override string ToString()
        {
            return $"Native Property of name '{Name}', "
                 + $"Return Type of {ReturnType}, "
                 + $"Parameter Type of {SelfArgument}, "
                 + $"Getter given by {Function.Method}";
        }
    }

    #endregion

    public class TaskCall(IExpression name, List<IExpression> arguments) : IExpression
    {
        public IExpression Name { get; } = name;
        public List<IExpression> Arguments { get; } = arguments;
    }

    public class TaskReturnSignal(object? value) : Exception
    {
        public object? Value { get; } = value;
    }

    /// <summary>
    /// Defines what kind of enum node we are referencing
    /// </summary>
    public enum EnumNodeKind { Definition, Member }
    public readonly record struct EnumNode(EnumNodeKind Kind, string Path, IReadOnlyList<EnumMember> Children);

    public interface IIterable
    {
    }

    public class List(MaxType? elementType) : IIterable
    {
        public List<object> Values { get; } = [];

        public MaxType? ElementType { get; private set; } = elementType;

        public void SetElementType(MaxType? elementType)
        {
            ElementType ??= elementType;
        }

        public override string ToString()
        {
            return $"[{string.Join(", ", Values)}]";
        }
    }

    public class Dictionary(MaxType? keyType, MaxType? valueType, bool duplicatesAllowed = false) : IIterable
    {
        
        public List<(object, object)> Entries { get; } = [];

        public MaxType? KeyType { get; } = keyType;
        public MaxType? ValueType { get; } = valueType;

        /// <summary>
        /// Whether the dictionary can have duplicates or not
        /// </summary>
        public bool DuplicatesAllowed { get; private set; } = duplicatesAllowed;

        public override string ToString()
        {
            return $"{{{string.Join(", ", Entries)}}}";
        }

        public static bool IsCompatibleKey(object? key) => key is string or int;

        public void SetDuplicates(bool boolean)
        {
            DuplicatesAllowed = boolean;
        }
    }

    #endregion
}