// This module is for utilities with warnings or errors

/// A struct that simply holds utility functions needed for casing.
/// This does not carry any fields, and only implements helpers
pub struct Casing;
impl Casing {
    pub fn is_pascal_case(name: &str) -> bool {
        let mut chars = name.chars();
        match chars.next() {
            // Only passes when the first letter is uppercase and the rest are alphanumeric
            Some(first) if first.is_uppercase() => chars.all(char::is_alphanumeric),
            _ => false,
        }  
    }

    pub fn to_pascal_case(name: &str) -> String {
        if Self::is_pascal_case(name) {
            return name.to_owned();
        }

        let split = name.split(|c: char| !c.is_alphanumeric());
        // underscores and other symbols become word breaks
        let filtered = split.filter(|word| !word.is_empty());
        let mapped = filtered
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                    None => String::new(),
                }
            })
            .collect();
        mapped
    }
}