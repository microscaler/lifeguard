//! Utility functions for code generation

use syn::spanned::Spanned;

/// Field identifier for named struct fields; tuple fields yield a compile-time error.
pub fn field_ident(field: &syn::Field) -> Result<&syn::Ident, syn::Error> {
    field.ident.as_ref().ok_or_else(|| {
        syn::Error::new(
            field.span(),
            "this derive only supports structs with named fields (tuple struct fields have no name)",
        )
    })
}

/// Convert string to `snake_case`
pub fn snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Convert string to `PascalCase`
pub fn pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert string to camelCase
/// 
/// This function is a placeholder for future functionality that may need
/// camelCase conversion (e.g., for JavaScript/TypeScript code generation).
#[allow(dead_code)]
pub fn camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap_or(c));
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
