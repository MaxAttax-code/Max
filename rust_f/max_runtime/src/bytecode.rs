use std::collections::HashMap;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use serde::Deserialize;

use crate::vm::MaxType;


// ---------------- \\
// --- OPERANDS --- \\
// ---------------- \\

/// The operand inside a Store instruction
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StoreOperand {
    pub name: String,
    pub modifiers: Vec<String>,
    #[serde(rename = "Type")]
    pub max_type: Option<MaxType>,
    #[serde(rename = "ResortValue")]
    pub resort_value: Option<serde_json::Value>
}

/// The operand inside an Update instruction
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateOperand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub operation: String
}

/// An operand that allows both indexing and updating the index
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateIndexOperand {
    #[serde(default)]
    pub operation: String,
    #[serde(rename = "TargetName")]
    pub target_name: Option<String>
}

/// A structure that holds a Variable Entry contained with the scopes of the Virtual Machine
/// Includes, the value as well as the type or resort value if considered
#[derive(Debug, Clone, PartialEq)]
pub struct VariableEntry {
    /// The value is stored within an Rc smart pointer. Please keep that in mind.
    pub value: std::rc::Rc<std::cell::RefCell<crate::vm::Value>>,
    pub modifiers: Vec<String>,
    pub max_type: Option<MaxType>,
    pub resort_value: Option<crate::vm::Value>,
}

impl VariableEntry {
    pub fn new(nvalue: crate::vm::Value, modifiers: Vec<String>, typing: Option<MaxType>, resortable: Option<crate::vm::Value>) -> Self {
        Self {
            value: Rc::new(RefCell::new(nvalue)),
            modifiers: modifiers,
            max_type: typing,
            resort_value: resortable
        }
    }
}

/// An operand that helps a Foreach loop work
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ForeachOperand {
    pub collection_slot: u64,
    pub index_slot: u64,
    pub declarations: Vec<String>,
    pub variable_type: Option<MaxType>
}

/// An operand that gives the member access info
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MemberAccessOperand {
    pub member_name: String,
    pub access_type: String
}


// -------------------- \\
// --- TASK CALLING --- \\
// -------------------- \\


/// The structure of a parameter within Task creation
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Parameter {
    #[serde(rename = "Type")]
    pub max_type: Option<MaxType>,
    pub name: String
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TaskReference {
    pub target: Option<String>, // Optional field for the library/target name being called
    pub name: String
}

/// An operand that feeds us the info to create a task
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TaskInfo {
    pub entry_point: u64,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<MaxType>
}

/// An operand that provides the needed requirements to call a function
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TaskCallOperand {
    pub target: Option<String>, // This is an optional field for the library/target name
    pub name: String,
    pub argument_count: u64,
    pub is_member_access: bool
}


// ------------------- \\
// ------ ENUMS ------ \\
// ------------------- \\


/// A single enum member, exactly as the compiler emits it.
/// Used by the VM at runtime as well, so no conversion is needed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EnumMember {
    // So many fields, gotta keep track of them all
    /// The direct name of the enum member, unique to it's members category
    pub name: String,
    /// The enumeration value given
    pub enumerable: i64,
    /// The primary, top-level enum this belongs to
    pub enum_name: String,
    /// The path to this enum, includes going through other members to reach a sub-member
    pub path: Vec<String>,
    #[serde(default)]
    /// The sub members this enum splits into
    pub sub_members: Vec<EnumMember>,
    #[serde(default)]
    /// A reference to another enum that this member is sub-split into.
    /// Keep in mind that if this is not **None**, `sub_members` must be an empty vector
    pub sub_enum_reference: Option<String>,
}

impl EnumMember {
    /// The Enum name attached with the path to this member
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.enum_name, self.path.join("."))
    }

    /// Same enum *and* same route inside it. Deliberately ignores `enumerable` and sub-members
    pub fn is_same_member(&self, other: &EnumMember) -> bool {
        self.enum_name == other.enum_name && self.path == other.path
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EnumStructure {
    pub identifier: String,
    pub enum_members: Vec<EnumMember>,
}


// ------------------- \\
// ----- LAMBDAS ----- \\
// ------------------- \\


/// The VMs representation of the Bytecode Chunk class from the C# Compiler
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Chunk {
    pub instructions: std::sync::Arc<Vec<Instruction>>,
    pub constants: Vec<serde_json::Value>
}

/// An operand that enters a lambda into the VM. Lambdas are stored with an ID
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LambdaOperand {
    #[serde(rename = "ID")]
    pub id: u64,
    pub arity: u64,
    #[serde(default)]
    pub parameters: Vec<String>,
    pub captured: Vec<String>,
    pub chunk: std::sync::Arc<Chunk>
}


// ------------------ \\
// --- COROUTINES --- \\
// ------------------ \\


#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    Idle,
    Running,
    Paused,
    Dead
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoroutineInstance {
    pub execution_state: ExecutionState,
    pub return_type: Option<MaxType>,

    /// Where the coroutine currently is. Starts at it's entry point.
    pub instruction_point: u64,

    /// Where the coroutine will start. Never changes
    pub entry_point: u64,

    /// The scopes the coroutine has travelled through.
    /// When the coroutine resumes, it begins from the last saved scope in this list
    pub saved_scopes: Option<Vec<std::sync::Arc<HashMap<String, VariableEntry>>>>,

    pub module_name: String
}

/// A type that holds an Rc Reference to a Coroutine Instance
/// This allows the coroutine to be borrowed and shared rather than cloned
pub type CoroutineHandle = Rc<RefCell<CoroutineInstance>>;

impl CoroutineInstance {
    /// Runs the coroutine. Call this function only when this coroutine is Paused or Idle
    pub fn run(&mut self) {
        self.execution_state = ExecutionState::Running;
    }

    /// Pauses the coroutine. Call this function only when this coroutine is Running
    pub fn pause(&mut self) {
        self.execution_state = ExecutionState::Paused;
    }

    /// Cancels the coroutine, putting it into Idle.
    /// Call this function only when this coroutine is Running or Paused
    pub fn cancel(&mut self) {
        self.execution_state = ExecutionState::Idle;
    }

    /// Kills the current coroutine, rendering it a dead instance.
    /// This function can be called whenever.
    pub fn terminate(&mut self) {
        self.execution_state = ExecutionState::Dead;
    }
}

// Default values for a coroutine instance
impl Default for CoroutineInstance {
    fn default() -> Self {
        Self {
            execution_state: ExecutionState::Idle,
            return_type: None,
            instruction_point: 0,
            entry_point: 0,
            saved_scopes: None,
            module_name: String::new()
        }
    }
}

/// An operand that provides the info to create a new coroutine instance
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct CoroutineInfo {
    pub entry_point: u64,
    pub return_type: Option<MaxType>
}

// ------------ \\
// --- MAIN --- \\
// ------------ \\

/// A single instruction emitted by the C# compiler
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Instruction {
    pub op_code: String,
    // Using serde_json::Value allows 'operand' to be a String, Object, or null dynamically
    pub operand: Option<serde_json::Value>
}

/// The Bytecode object that contains all instructions, tasks, etc..., for the VM to read.
/// All fields are wrapped in Arcs in order to allow easy cloning from Bytecode to Bytecode.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Bytecode {
    pub instructions: Arc<Vec<Instruction>>,
    pub tasks: Arc<HashMap<String, TaskInfo>>,
    pub coroutines: Arc<HashMap<String, CoroutineInfo>>,
    pub enums: Arc<HashMap<String, EnumStructure>>
}