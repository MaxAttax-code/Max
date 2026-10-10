// Under license to the reader by the author of the Max Language®
// Relaying these files and/or distributing or publishing them without the author's consent may incur a fine

namespace Max
{
    // All these classes are just Exceptions (i.e. raise errors in the code when called)
    public class MaxSyntaxException(string message) : Exception(message)
    {
    }

    public class MaxArithmeticException(string message) : Exception(message)
    {
    }

    public class MaxComparisonException(string message) : Exception(message)
    {
    }

    public class MaxVariableException(string message) : Exception(message)
    {
    }

    public class MaxTaskException(string message) : Exception(message)
    {
    }

    public class MaxNilException(string message) : Exception(message)
    {
    }

    public class MaxEndFileException(string message) : MaxSyntaxException(message)
    {
    }

    public class MaxValueException(string message) : Exception(message)
    {
    }

    public class MaxTypeException(string message) : MaxValueException(message)
    {
    }

    public class MaxLogicException(string message) : MaxComparisonException(message)
    {
    }

    public class MaxEndOfObjectException(string message) : Exception(message)
    {
    }

    public class MaxIndexingException(string message) : MaxEndOfObjectException(message)
    {
    }

    public class MaxInfiniteLoopException(string message) : Exception(message)
    {
    }

    public class MaxFileNotFoundException(string message) : Exception(message)
    {
    }
}