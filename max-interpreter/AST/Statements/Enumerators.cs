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
    }

    // More like a definition than a statement, but eh
    public class EnumeratorStatement(string identifier, List<EnumMember> enumMembers) : IStatement
    {
        public string Identifier { get; } = identifier;
        public List<EnumMember> EnumMembers { get; } = enumMembers;
    }
}