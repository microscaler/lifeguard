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
    /// should be converted to/from. For single-column primary keys, this is the
    /// column's type (with `Option<T>` unwrapped to `T`). For composite primary
    /// keys (multiple columns), this is a tuple of all primary key types.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyTrait;
    ///
    /// // For a single primary key with type i32:
    /// // type ValueType = i32;
    ///
    /// // For a single primary key with type Option<i32>:
    /// // type ValueType = i32;  // Option unwrapped
    ///
    /// // For a composite primary key (i32, String):
    /// // type ValueType = (i32, String);
    ///
    /// // For a composite primary key (Option<i32>, String):
    /// // type ValueType = (i32, String);  // Option unwrapped
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
/// This enum indicates the exact number of columns in a primary key, providing
/// better type safety than SeaORM's simple `Single`/`Tuple` distinction.
/// This enables compile-time verification of composite key sizes and more
/// specific handling for different arities.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyArity;
///
/// // Single column primary key
/// let arity = PrimaryKeyArity::Single;
///
/// // Composite primary keys with specific sizes
/// let arity2 = PrimaryKeyArity::Tuple2;  // 2 columns
/// let arity3 = PrimaryKeyArity::Tuple3;  // 3 columns
/// let arity4 = PrimaryKeyArity::Tuple4;  // 4 columns
/// let arity5 = PrimaryKeyArity::Tuple5;  // 5 columns
/// let arity6plus = PrimaryKeyArity::Tuple6Plus;  // 6+ columns
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKeyArity {
    /// Single column primary key
    Single,
    /// Composite primary key with 2 columns
    Tuple2,
    /// Composite primary key with 3 columns
    Tuple3,
    /// Composite primary key with 4 columns
    Tuple4,
    /// Composite primary key with 5 columns
    Tuple5,
    /// Composite primary key with 6 or more columns
    Tuple6Plus,
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
    /// - `PrimaryKeyArity::Tuple2` through `Tuple5` for composite keys with 2-5 columns
    /// - `PrimaryKeyArity::Tuple6Plus` for composite keys with 6 or more columns
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
    /// //     PrimaryKeyArity::Tuple2 => {
    /// //         // Handle 2-column composite key
    /// //     }
    /// //     PrimaryKeyArity::Tuple3 => {
    /// //         // Handle 3-column composite key
    /// //     }
    /// //     PrimaryKeyArity::Tuple6Plus => {
    /// //         // Handle 6+ column composite key
    /// //     }
    /// //     _ => {
    /// //         // Handle other arities
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
        let tuple2 = PrimaryKeyArity::Tuple2;
        let tuple3 = PrimaryKeyArity::Tuple3;
        let tuple4 = PrimaryKeyArity::Tuple4;
        let tuple5 = PrimaryKeyArity::Tuple5;
        let tuple6plus = PrimaryKeyArity::Tuple6Plus;
        
        assert_eq!(single, PrimaryKeyArity::Single);
        assert_eq!(tuple2, PrimaryKeyArity::Tuple2);
        assert_eq!(tuple3, PrimaryKeyArity::Tuple3);
        assert_eq!(tuple4, PrimaryKeyArity::Tuple4);
        assert_eq!(tuple5, PrimaryKeyArity::Tuple5);
        assert_eq!(tuple6plus, PrimaryKeyArity::Tuple6Plus);
        assert_ne!(single, tuple2);
        assert_ne!(tuple2, tuple3);
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
            PrimaryKeyArity::Tuple2
        }
    }

    #[test]
    fn test_composite_primary_key_arity() {
        assert_eq!(CompositePrimaryKey::arity(), PrimaryKeyArity::Tuple2);
    }
}
