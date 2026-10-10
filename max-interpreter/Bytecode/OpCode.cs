namespace Max
{
    public enum OpCode
    {
        Constant,
        MakeList,
        MakeDictionary,
        CreateCoroutine,
        Lambda,
        MemberAccess,

        Store,
        TemporaryStore,
        Update,
        Load,
        TemporaryLoad,
        ForeachNext,
        EnterScope,
        ExitScope,

        Throw,

        Add,
        Subtract,
        Multiply,
        Divide,
        Modulus,

        Compare,
        Index,
        /// <summary>
        /// Update an indexed value by first accessing the indexed value (duh)
        /// </summary>
        UpdateIndex,

        /// <summary>
        /// Call a user-defined task of it's operand.
        /// This instruction defaults to CallNative when it fails
        /// </summary>
        Call,

        /// <summary>
        /// Call a native task of it's operand
        /// </summary>
        CallNative,

        /// <summary>
        /// Simply returns and exits from the closest block
        /// </summary>
        Return,

        /// <summary>
        /// Returns and also brings a value alongside
        /// </summary>
        ReturnValue,

        /// <summary>
        /// Allows a coroutine to be paused, whilst returning a value in the process
        /// </summary>
        Yield,
        Pop,

        Not,
        Negate,

        /// <summary>
        /// An OpCode reference that allows the current instruction to jump to a certain point in the instructions list, i.e. the operand.
        /// </summary>
        Jump,

        /// <summary>
        /// A version of Jump that only occurs if the previous value was a boolean, and was false. Otherwise it continues.
        /// This is primarily for if statements
        /// </summary>
        JumpIfFalse,

        /// <summary>
        /// A specific jump pattern for if statements that, if it follows the possible value, jumps to the dedicated place.
        /// This is primarily for if statements
        /// </summary>
        JumpIfPossible,

        Say
    }
}