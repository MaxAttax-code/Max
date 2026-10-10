// Under license to the reader by the author of the Max Language®
// Relaying these files and/or distributing or publishing them without the author's consent may incur a fine

namespace Max
{
    public class IntegerLiteral(int value) : IExpression
    {
        public int Value { get; } = value;
    }

    public class FloatLiteral(double value) : IExpression
    {
        public double Value { get; } = value;
    }

    public class StringLiteral(string value) : IExpression
    {
        public string Value { get; } = value;
    }

    public class BooleanLiteral(bool value) : IExpression
    {
        public bool Value { get; } = value;
    }

    public class ListLiteral(List<IExpression> values) : IExpression
    {
        public List<IExpression> Values { get; } = values;
    }

    public class DictionaryLiteral(List<(IExpression Key, IExpression Value)> entries) : IExpression
    {
        public List<(IExpression Key, IExpression Value)> Entries { get; } = entries;
    }

    public class PossibleLiteral : IExpression
    {
        // Yes it gets it's own specific value, so it can be printed
        public string Value { get; } = "possible";
        public override string ToString() => "possible";
    }

    public class NilLiteral : IExpression
    {
        public string Value { get; } = "nil";
        public override string ToString() => "nil";
    }

    public class VariableReference(string name) : IExpression
    {
        public string Name { get; } = name;
    }

    public class Parameter(MaxType? type, string name) : IExpression
    {
        public MaxType? Type { get; } = type;
        public string Name { get; } = name;
    }
}