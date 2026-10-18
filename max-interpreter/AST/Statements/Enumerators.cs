namespace Max
{
    /// <summary>
    /// An Enum member in Max
    /// </summary>
    /// <param name="Name">The name of the enum that will be referenced</param>
    /// <param name="Enumerable">It'd index with in it's enumeration</param>
    public record EnumMember(string Name, int Enumerable);

    // More like a definition than a statement, but eh
    public class EnumeratorStatement(string identifier, List<EnumMember> enumMembers) : IStatement
    {
        public string Identifier { get; } = identifier;
        public List<EnumMember> EnumMembers { get; } = enumMembers;
    }
}