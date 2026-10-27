use std::{collections::HashMap, fmt, sync::Arc};

/// An enum of every OpCode
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Constant(ConstValue),
    MakeList(u32),
    MakeDictionary(u32),
    CreateCoroutine(CoroutineInfo),
    DefineEnum(EnumStructure),
    Lambda(Arc<LambdaOperand>),
    MemberAccess(MemberAccessOperand),

    Store(StoreOperand),
    TemporaryStore(u64),
    Update(UpdateOperand),
    Load(String),
    TemporaryLoad(u64),
    ForeachNext(ForeachOperand),
    EnterScope,
    ExitScope,

    Throw(String),

    Add,
    Subtract,
    Multiply,
    Divide,
    Modulus,

    Compare(CompareOp),
    Index,
    UpdateIndex(UpdateIndexOperand),

    Call(TaskCallOperand),
    CallNative(TaskCallOperand),
    Return,
    ReturnValue,
    Yield,
    Pop,

    Not,
    Negate,

    And,
    Or,

    Jump(usize),
    JumpIfFalse(usize),
    JumpIfPossible(usize),

    Say,
}

/*
    ----- FLAG TYPES -----
*/

// It's easier to make these options as enums rather than string choices

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp { Equal, NotEqual, Lesser, LesserEqual, Greater, GreaterEqual }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOperation { Assign, Add, Subtract, Multiply, Divide, Modulus }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAccessType { Module, Enum, EnumMember, None }

/// A struct rather than an enum, as multiple modifiers may be active at onces
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers { pub open: bool, pub is_static: bool, pub fixed: bool }

impl Modifiers { pub fn e_new() -> Self { Modifiers { open: false, is_static: false, fixed: false } } }

#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Integer(i64),
    Float(f64),
    Str(String),
    Boolean(bool),
    Possible,
    Nil,
    List(Vec<ConstValue>),
    Dictionary(Vec<(ConstValue, ConstValue)>)   
}


// -------------------- \\
// -- TYPES & VALUES -- \\
// -------------------- \\

/*
    ----------------------
    ----- TYPE NAMES -----
    ----------------------
*/

/// An enum of all possible `MaxType`
#[derive(Debug, Clone, PartialEq)]
pub enum MaxType {
    NumberType,
    IntegerType,
    FloatType,

    StringType,

    BooleanType,
    PossibleType,

    ListType(Option<Box<MaxType>>),
    DictionaryType(Option<Box<MaxType>>, Option<Box<MaxType>>),

    TaskType(usize), // I.e. Lambdas
    CoroutineType,

    EnumType(String), // An EnumType has a specific identifier unique to it

    // A special type for niche situations within the code
    NoType
}

impl MaxType {    

    /// Compares if the max types of this and another type are the same. This ignores values within the Variant Type
    pub fn compare_valuetypes(&self, other: &MaxType) -> bool {
        // Like Value logic but with types
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /**
        Returns true if the `actual` type can be assigned to a variable expecting `self
        ### Example
        IntegerTypes are assignable to FloatTypes

        ```
        let integerType = MaxType::IntegerType;

        println!("{}", integerType.is_assignable_to(MaxType::FloatType)) // Prints true
        ```
    */
    pub fn is_assignable_to(&self, actual: &MaxType) -> bool {
        // When we are just comparing a type to itself
        if self == actual {
            return true;
        }

        match (self, actual) {
            // NumberType accepts both IntegerType and FloatType
            (MaxType::NumberType, MaxType::IntegerType) => true,
            (MaxType::NumberType, MaxType::FloatType) => true,

            // A FloatType can accept an IntegerType (but not vice versa)
            (MaxType::FloatType, MaxType::IntegerType) => true,

            // A possible type is also a Boolean-Like
            (MaxType::BooleanType, MaxType::PossibleType) => true,
            
            // Nested type checking for lists (E.g. A list of numbers are both ints and floats; This prevents weird type errors)
            (MaxType::ListType(Some(expected_elem)), MaxType::ListType(Some(actual_elem))) => {
                expected_elem.is_assignable_to(actual_elem)
            }

            (MaxType::DictionaryType(Some(expected_key), Some(expected_val)), MaxType::DictionaryType(Some(actual_key), Some(actual_val))) => {
                expected_key.is_assignable_to(actual_key) && expected_val.is_assignable_to(actual_val)
            }
            
            _ => false,
        }
    }

    /// Checks if a given type is valid to be a dictionary key
    pub fn valid_dictkey_type(&self, value: &MaxType) -> bool {
        match self {
            // Pretty cool, '..' means just ignore what's inside
            MaxType::DictionaryType( .. ) => {
                if matches!(value, MaxType::StringType | MaxType::IntegerType) {
                    return true
                }

                false
            }

            _ => { println!("Can only call this function on dictionaries!"); false }
        }
    }

    pub fn string_name_to_max(name: &str) -> MaxType {
        match name {
            "NumberType" => MaxType::NumberType,
            "IntegerType" => MaxType::IntegerType,
            "FloatType" => MaxType::FloatType,

            "StringType" => MaxType::StringType,

            "BooleanType" => MaxType::BooleanType,
            "PossibleType" => MaxType::PossibleType,

            "ListType" => MaxType::ListType(None),
            "DictionaryType" => MaxType::DictionaryType(None, None),

            "CoroutineType" => MaxType::CoroutineType,
            "TaskType" => MaxType::TaskType(0),

            "NoType" => MaxType::NoType,

            _ => panic!("Unknown MaxType name given: {}", name)
        }
    }
}

impl fmt::Display for MaxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write out the name of the Type
        match self {
            MaxType::NumberType => write!(f, "NumberType"),
            MaxType::IntegerType => write!(f, "IntegerType"),
            MaxType::FloatType => write!(f, "FloatType"),

            MaxType::StringType => write!(f, "StringType"),

            MaxType::BooleanType => write!(f, "BooleanType"),
            MaxType::PossibleType => write!(f, "PossibleType"),

            MaxType::NoType => write!(f, "NoType"),

            // Lists can also write out their element type
            MaxType::ListType(elem) => match elem {
                Some(inner) => write!(f, "ListType<{}>", inner),

                None => write!(f, "ListType"),
            },

            // Dictionaries can also write out the key/value types
            MaxType::DictionaryType(key, val) => match (key, val) {
                (Some(k), Some(v)) => write!(f, "DictionaryType<{}, {}>", k, v),

                _ => write!(f, "DictionaryType"),
            },

            MaxType::CoroutineType => write!(f, "CoroutineType"),
            MaxType::TaskType(_) => write!(f, "TaskType"),

            MaxType::EnumType(enum_name) => write!(f, "{}", enum_name)
        }
    }
}

// ---------------- \\
// --- OPERANDS --- \\
// ---------------- \\

/// The operand inside a Store instruction
#[derive(Debug, Clone, PartialEq)]
pub struct StoreOperand {
    pub name: String,
    pub modifiers: Modifiers,
    pub max_type: Option<MaxType>,
    pub resort_value: Option<ConstValue>
}

/// The operand inside an Update instruction
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOperand {
    pub name: String,
    pub operation: UpdateOperation
}

/// An operand that allows both indexing and updating the index
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateIndexOperand {
    pub operation: UpdateOperation,
    pub target_name: Option<String>
}

/// An operand that helps a Foreach loop work
#[derive(Debug, Clone, PartialEq)]
pub struct ForeachOperand {
    pub collection_slot: u64,
    pub index_slot: u64,
    pub declarations: Vec<String>,
    pub variable_type: Option<MaxType>
}

/// An operand that gives the member access info
#[derive(Debug, Clone, PartialEq)]
pub struct MemberAccessOperand {
    pub member_name: String,
    pub access_type: MemberAccessType
}


// -------------------- \\
// --- TASK CALLING --- \\
// -------------------- \\


/// The structure of a parameter within Task creation
#[derive(Debug, Clone)]
pub struct Parameter {
    pub max_type: Option<MaxType>,
    pub name: String
}

/// An operand that feeds us the info to create a task
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub entry_point: u64,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<MaxType>
}

/// An operand that provides the needed requirements to call a function
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
    /// The sub members this enum splits into
    pub sub_members: Vec<EnumMember>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct EnumStructure {
    pub identifier: String,
    pub enum_members: Vec<EnumMember>,
}


// ------------------- \\
// ----- LAMBDAS ----- \\
// ------------------- \\


/// The VMs representation of the Bytecode Chunk class from the C# Compiler
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub instructions: Arc<Vec<Op>>
}

/// An operand that enters a lambda into the VM. Lambdas are stored with an ID
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaOperand {
    pub id: u64,
    pub arity: u64,
    pub parameters: Vec<String>,
    pub captured: Vec<String>,
    pub chunk: std::sync::Arc<Chunk>
}


// ------------------ \\
// --- COROUTINES --- \\
// ------------------ \\

/// An operand that provides the info to create a new coroutine instance
#[derive(Debug, Clone, PartialEq)]
pub struct CoroutineInfo {
    pub entry_point: u64,
    pub return_type: Option<MaxType>
}

// ------------ \\
// --- MAIN --- \\
// ------------ \\

/// The Bytecode object that contains all instructions, tasks, etc..., for the VM to read.
/// All fields are wrapped in Arcs in order to allow easy cloning from Bytecode to Bytecode.
#[derive(Debug, Clone)]
pub struct Bytecode {
    pub instructions: Arc<Vec<Op>>,
    pub tasks: Arc<HashMap<String, TaskInfo>>,
    pub coroutines: Arc<HashMap<String, CoroutineInfo>>,
    pub enums: Arc<HashMap<String, EnumStructure>>
}