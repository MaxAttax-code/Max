namespace Max
{
    public class BinaryExpression(IExpression left, string operation, IExpression right) : IExpression
    {
        public IExpression Left { get; } = left;
        public string Operator { get; } = operation;
        public IExpression Right { get; } = right;
    }

    public class ComparisonExpression(IExpression left, string operation, IExpression right) : IExpression
    {
        public IExpression Left { get; } = left;
        public string Operator { get; } = operation;
        public IExpression Right { get; } = right;

        public override string ToString()
        {
            return $"Left hand: {Left}, " +
                   $"Operator: {Operator}, " +
                   $"Right hand: {Right}";
        }
    }

    public class UnaryExpression(UnaryOperator @operator, IExpression operand) : IExpression
    {
        public UnaryOperator Operator { get; } = @operator;
        public IExpression Operand { get; } = operand;
    }

    public class LogicalExpression(IExpression left, LogicalOperator @operator, IExpression right) : IExpression
    {
        public IExpression Left { get; } = left;
        public LogicalOperator Operator { get; } = @operator;
        public IExpression Right { get; } = right;
    }

    public class IndexingExpression(IExpression name, IExpression indexValue) : IExpression
    {
        public IExpression Name { get; } = name;
        public IExpression IndexValue { get; } = indexValue;
    }

    public class MemberAccessExpression(IExpression target, string member) : IExpression
    {
        public IExpression Target { get; } = target;
        public string Member { get; } = member;
    }

    public class LambdaExpression(List<string> @params, List<IStatement>? body = null, IExpression? single = null) : IExpression
    {
        public List<string> Parameters { get; } = @params;
        /// <summary>
        /// Can be either a simple expression or an entire body!
        /// </summary>
        public IExpression? Single { get; } = single;
        /// <summary>
        /// You cannot have both a body and single liner inside a lambda expression!
        /// </summary>
        public List<IStatement>? Body { get; } = body;
    }
}