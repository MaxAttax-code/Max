namespace Max
{
    public enum TaskType
    {
        Function,
        Coroutine
    }

    /// <summary>
    /// A class to represent the different modifiers placed on a declaration.
    /// Only one modifier at a time can take up it's type in the container.
    /// </summary>
    public class ModifierSet
    {
        public LifecycleModifier? LifecycleModifier { get; set; }
        public MutabilityModifier? MutabilityModifier { get; set; }

        /// <summary>
        /// Converts all the modifiers that are NON-NULL into a hashset.
        /// The order of the hashset is determined by the order of the modifier fields above.
        /// </summary>
        /// <returns></returns>
        public HashSet<string> ToHashSet()
        {
            HashSet<string> set = [];

            if (LifecycleModifier != null)
                set.Add(LifecycleModifier.ToString()!.ToLower());

            if (MutabilityModifier != null)
                set.Add(MutabilityModifier.ToString()!.ToLower());

            return set;
        }
    }

    public enum LifecycleModifier
    {
        Open,
        Static
    }

    public enum MutabilityModifier
    {
        Fixed
    }

    /// <summary>
    /// Defines what kind of member accessing we are doing
    /// </summary>
    public enum MemberAccessType
    {
        Module,
        Enum,
        EnumMember,

        None // Represents that there should be no member accessing
    }
}