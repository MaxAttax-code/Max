use serde::Serialize;

#[derive(Serialize)]
pub struct NativeTaskSpec {
    pub name: String,
    pub is_property: bool
}

#[derive(Serialize)]
pub struct NativeModuleSpec {
    pub name: String,
    pub tasks: Vec<NativeTaskSpec>
}

#[derive(Serialize)]
pub struct NativeRegistrySpec {
    pub static_modules: Vec<NativeModuleSpec>,
    pub global_tasks: Vec<NativeTaskSpec>
}