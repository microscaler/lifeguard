//! Primary key operations for LifeModel - Epic 02 Story 06
//!
//! This module provides traits and implementations for type-safe primary key operations
//! that match SeaORM's API. Primary keys can be queried and manipulated with compile-time type checking.

/// Trait for primary key operations
///
/// This trait provides methods for working with primary keys at compile time.
/// It's similar to SeaORM's `PrimaryKeyTrait` and allows type-safe primary key operations.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyTrait;
///
/// // In a real application, the macro would generate this:
/// // impl PrimaryKeyTrait for UserPrimaryKey {
/// //     type ValueType = i32;
/// //     fn auto_increment(self) -> bool {
/// //         match self {
/// //             UserPrimaryKey::Id => true,
/// //         }
/// //     }
/// // }
/// ```
pub trait PrimaryKeyTrait: Copy + std::fmt::Debug {
    /// The value type of the primary key
    ///
    /// This associated type represents the Rust type that the primary key value
    /// should be converted to/from. For example, `i32` for integer primary keys,
    /// `String` for string primary keys, etc.
    ///
    /// # Limitations
    ///
    /// **Composite Primary Keys:** For composite primary keys (multiple columns),
    /// `ValueType` currently only tracks the type of the **first** primary key column.
    /// This is a known limitation. Full composite key support would require a tuple
    /// type (e.g., `(i32, String)`), which is a future enhancement.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyTrait;
    ///
    /// // For a primary key with type i32:
    /// // type ValueType = i32;
    ///
    /// // For a composite primary key (i32, String):
    /// // type ValueType = i32;  // Only first key's type (limitation)
    /// ```
    type ValueType;

    /// Check if the primary key is auto-increment
    ///
    /// Returns `true` if the primary key column has the `#[auto_increment]` attribute,
    /// `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyTrait;
    ///
    /// // In a real application, the macro would generate this:
    /// // impl PrimaryKeyTrait for UserPrimaryKey {
    /// //     fn auto_increment(self) -> bool {
    /// //         match self {
    /// //             UserPrimaryKey::Id => true,  // Has #[auto_increment]
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn auto_increment(self) -> bool;
}

/// Trait for mapping between PrimaryKey and Column
///
/// This trait provides a way to convert a PrimaryKey enum variant to its
/// corresponding Column enum variant. This is useful for queries that need
/// to reference the primary key column.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyToColumn;
///
/// // In a real application, the macro would generate this:
/// // impl PrimaryKeyToColumn for UserPrimaryKey {
/// //     type Column = UserColumn;
    /// //     fn to_column(self) -> Self::Column {
    /// //         match self {
    /// //             UserPrimaryKey::Id => UserColumn::Id,
    /// //         }
    /// //     }
    /// // }
/// ```
pub trait PrimaryKeyToColumn {
    /// The Column enum type for this entity
    type Column;

    /// Convert a PrimaryKey variant to its corresponding Column variant
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyToColumn;
    ///
    /// // let pk = UserPrimaryKey::Id;
    /// // let col = pk.to_column();  // Returns UserColumn::Id
    /// ```
    fn to_column(self) -> Self::Column;
}

/// Enum representing the arity (number of columns) in a primary key
///
/// This enum indicates whether a primary key consists of a single column
/// or multiple columns (composite key). It's used to determine how to
/// handle primary key operations, especially for composite keys.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyArity;
///
/// // Single column primary key
/// let arity = PrimaryKeyArity::Single;
///
/// // Composite primary key (multiple columns)
/// let arity = PrimaryKeyArity::Tuple;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKeyArity {
    /// Single column primary key
    Single,
    /// Composite primary key (multiple columns)
    Tuple,
}

/// Trait for determining the arity of a primary key
///
/// This trait provides a method to determine whether a primary key
/// consists of a single column or multiple columns (composite key).
/// This is essential for proper handling of composite primary keys
/// in operations like `get_primary_key_value()`.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{PrimaryKeyArity, PrimaryKeyArityTrait};
///
/// // In a real application, the macro would generate this:
/// // impl PrimaryKeyArityTrait for UserPrimaryKey {
/// //     fn arity() -> PrimaryKeyArity {
/// //         PrimaryKeyArity::Single  // Single column
/// //     }
/// // }
///
/// // For a composite primary key:
/// // impl PrimaryKeyArityTrait for CompositePrimaryKey {
/// //     fn arity() -> PrimaryKeyArity {
/// //         PrimaryKeyArity::Tuple  // Multiple columns
/// //     }
/// // }
/// ```
pub trait PrimaryKeyArityTrait {
    /// Returns the arity of the primary key
    ///
    /// - `PrimaryKeyArity::Single` for single-column primary keys
    /// - `PrimaryKeyArity::Tuple` for composite (multi-column) primary keys
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{PrimaryKeyArity, PrimaryKeyArityTrait};
    ///
    /// // let arity = UserPrimaryKey::arity();
    /// // match arity {
    /// //     PrimaryKeyArity::Single => {
    /// //         // Handle single column primary key
    /// //     }
    /// //     PrimaryKeyArity::Tuple => {
    /// //         // Handle composite primary key
    /// //     }
    /// // }
    /// ```
    fn arity() -> PrimaryKeyArity;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test PrimaryKey enum for PrimaryKeyTrait tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestPrimaryKey {
        Id,
    }

    // Test Column enum for PrimaryKeyToColumn tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestColumn {
        Id,
    }

    // Manual implementation for testing
    impl PrimaryKeyTrait for TestPrimaryKey {
        type ValueType = i32;

        fn auto_increment(self) -> bool {
            match self {
                TestPrimaryKey::Id => true,
            }
        }
    }

    impl PrimaryKeyToColumn for TestPrimaryKey {
        type Column = TestColumn;

        fn to_column(self) -> Self::Column {
            match self {
                TestPrimaryKey::Id => TestColumn::Id,
            }
        }
    }

    #[test]
    fn test_primary_key_trait_auto_increment() {
        let pk = TestPrimaryKey::Id;
        assert_eq!(pk.auto_increment(), true);
    }

    #[test]
    fn test_primary_key_to_column() {
        let pk = TestPrimaryKey::Id;
        let col = pk.to_column();
        assert_eq!(col, TestColumn::Id);
    }

    #[test]
    fn test_primary_key_value_type() {
        // Test that ValueType is accessible
        let _value: <TestPrimaryKey as PrimaryKeyTrait>::ValueType = 42i32;
    }

    // Test PrimaryKeyArity enum
    #[test]
    fn test_primary_key_arity_enum() {
        let single = PrimaryKeyArity::Single;
        let tuple = PrimaryKeyArity::Tuple;
        
        assert_eq!(single, PrimaryKeyArity::Single);
        assert_eq!(tuple, PrimaryKeyArity::Tuple);
        assert_ne!(single, tuple);
    }

    // Test PrimaryKeyArityTrait for single primary key
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum SinglePrimaryKey {
        Id,
    }

    impl PrimaryKeyArityTrait for SinglePrimaryKey {
        fn arity() -> PrimaryKeyArity {
            PrimaryKeyArity::Single
        }
    }

    #[test]
    fn test_single_primary_key_arity() {
        assert_eq!(SinglePrimaryKey::arity(), PrimaryKeyArity::Single);
    }

    // Test PrimaryKeyArityTrait for composite primary key
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum CompositePrimaryKey {
        Id1,
        Id2,
    }

    impl PrimaryKeyArityTrait for CompositePrimaryKey {
        fn arity() -> PrimaryKeyArity {
            PrimaryKeyArity::Tuple
        }
    }

    #[test]
    fn test_composite_primary_key_arity() {
        assert_eq!(CompositePrimaryKey::arity(), PrimaryKeyArity::Tuple);
    }
}
