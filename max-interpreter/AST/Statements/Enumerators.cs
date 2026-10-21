namespace Max
{
    /// <summary>
    /// An Enum member in Max
    /// </summary>
    /// <param name="Name">The name of the member that will be referenced</param>
    /// <param name="Parent">The name of the enum this member derives from</param>
    public record EnumMember(string Name, int Enumerable)
    {
        public List<EnumMember> SubMembers { get; init; } = [];
        /// <summary>
        /// An optional field that holds a reference to another enum as submembers
        /// </summary>
        public string? SubEnumReference { get; init; }
        /// <summary>
        /// A to-initialise field telling what the parent enum name is
        /// </summary>
        public string EnumName { get; init; } = "";
        /// <summary>
        /// A to-initialise field telling what the path to reach the enum member from the primary enum is.
        /// The list must be in the order of traversing to find the enum member.
        /// For example
        /// <code>
        /// // With context of a defined enum member of Primary.Secondary.Tertiary
        /// 
        /// EnumMember tertiaryMember = new EnumMember(
        ///     "Tertiary", 0
        /// ) {
        ///     EnumName = "Secondary",
        ///     Path = ["Secondary", "Tertiary"]
        /// };
        /// </code>
        /// </summary>
        public List<string> Path { get; init; } = [];
    }

    // More like a definition than a statement, but eh
    public class EnumeratorStatement(string identifier, List<EnumMember> enumMembers) : IStatement
    {
        public string Identifier { get; } = identifier;
        public List<EnumMember> EnumMembers { get; } = enumMembers;
    }
}