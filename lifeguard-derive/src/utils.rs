//! Utility functions for code generation

/// Convert string to snake_case
pub fn snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

/// Convert string to PascalCase
pub fn pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert string to camelCase
pub fn camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert_eq!(snake_case("UserId"), "user_id");
        assert_eq!(snake_case("user_id"), "user_id");
        assert_eq!(snake_case("User"), "user");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(pascal_case("user_id"), "UserId");
        assert_eq!(pascal_case("user"), "User");
        assert_eq!(pascal_case("_user_id"), "UserId");
    }

    #[test]
    fn test_camel_case() {
        assert_eq!(camel_case("user_id"), "userId");
        assert_eq!(camel_case("user"), "user");
        assert_eq!(camel_case("_user_id"), "UserId");
    }
}
