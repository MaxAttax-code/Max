namespace Max
{
    public class NativeTaskSpec
    {
        public required string Name { get; set; }
        public required bool IsProperty { get; set; }
    }

    public class NativeModuleSpec
    {
        public required string Name { get; set; }
        public required List<NativeTaskSpec> Tasks { get; set; }
    }

    public class NativeRegistrySpec
    {
        public required List<NativeModuleSpec> StaticModules { get; set; }
        public required List<NativeTaskSpec> GlobalTasks { get; set; }
    }
}