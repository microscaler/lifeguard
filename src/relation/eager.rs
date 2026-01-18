//! Eager loading utilities for related entities.
//!
//! This module provides utilities for loading related entities eagerly,
//! similar to SeaORM's `selectinload` strategy. It loads related entities
//! in a separate optimized query after fetching the main entities.
//!
//! # Example
//!
//! ```no_run
//! use lifeguard::{load_related, LifeModelTrait, LifeExecutor, Related};
//!
//! // Fetch users
//! # struct UserModel { id: i32 };
//! # struct PostModel { id: i32, user_id: i32 };
//! # let users: Vec<UserModel> = vec![];
//! # let executor: &dyn LifeExecutor = todo!();
//!
//! // Eagerly load posts for all users
//! let posts_by_user = load_related::<UserModel, PostModel>(&users, executor)?;
//! // posts_by_user is a HashMap mapping user IDs to their posts
//! ```
//!
//! # Strategy
//!
//! This uses the "selectinload" strategy:
//! 1. Fetch main entities (e.g., users)
//! 2. Extract primary keys from main entities
//! 3. Make a single optimized query to fetch all related entities
//! 4. Group related entities by their parent entity's primary key
//!
//! This is more efficient than N+1 queries (one query per main entity).

use crate::executor::{LifeExecutor, LifeError};
use crate::model::ModelTrait;
use crate::query::{SelectQuery, LifeModelTrait};
use crate::relation::traits::{Related, FindRelated};
use crate::relation::def::{RelationDef, extract_table_name};
use crate::relation::identity::Identity;
use sea_query::{Expr, ExprTrait, Condition, ConditionType};
use std::collections::HashMap;

/// Load related entities for a collection of main entities
///
/// This function implements eager loading using the "selectinload" strategy:
/// 1. Extracts primary keys from the main entities
/// 2. Makes a single optimized query to fetch all related entities
/// 3. Groups related entities by their parent entity's primary key
///
/// # Type Parameters
///
/// * `M` - The main model type (e.g., `UserModel`)
/// * `R` - The related model type (e.g., `PostModel`)
///
/// # Arguments
///
/// * `entities` - A slice of main entities to load related entities for
/// * `executor` - The database executor to use
///
/// # Returns
///
/// Returns a `HashMap` mapping primary key values (as `String`) to vectors of related entities.
/// The key is a string representation of the primary key (supports composite keys).
///
/// # Known Limitations
///
/// **⚠️ This is a partial implementation.** The query building is correct and efficient,
/// but the grouping logic requires FK value extraction from related entities, which needs
/// additional infrastructure (either a helper trait or column name-based access).
/// Currently, all related entities are grouped under the first parent entity as a placeholder.
///
/// To complete this implementation, one of the following is needed:
/// 1. Add a method to `ModelTrait` to get values by column name string
/// 2. Use serialization (if models implement `Serialize`)
/// 3. Require a user-provided mapping function
/// 4. Use a helper trait extension
///
/// # Example
///
/// ```no_run
/// use lifeguard::{load_related, LifeModelTrait, LifeExecutor, Related};
///
/// # struct UserModel { id: i32 };
/// # struct PostModel { id: i32, user_id: i32 };
/// # impl lifeguard::ModelTrait for UserModel {
/// #     type Entity = User;
/// #     fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
/// #     fn get_primary_key_identity(&self) -> lifeguard::Identity { todo!() }
/// #     fn get_primary_key_values(&self) -> Vec<sea_query::Value> { todo!() }
/// #     fn get(&self, _col: <User as lifeguard::LifeModelTrait>::Column) -> sea_query::Value { todo!() }
/// #     fn set(&mut self, _col: <User as lifeguard::LifeModelTrait>::Column, _val: sea_query::Value) -> Result<(), lifeguard::ModelError> { todo!() }
/// # }
/// # struct User;
/// # impl lifeguard::LifeModelTrait for User {
/// #     type Model = UserModel;
/// #     type Column = ();
/// # }
/// # struct Post;
/// # impl lifeguard::LifeModelTrait for Post {
/// #     type Model = PostModel;
/// #     type Column = ();
/// # }
/// # let users: Vec<UserModel> = vec![];
/// # let executor: &dyn LifeExecutor = todo!();
///
/// // Load posts for all users
/// let posts_by_user = load_related::<UserModel, PostModel>(&users, executor)?;
///
/// // Access posts for a specific user
/// let user_id = "1"; // Primary key as string
/// if let Some(posts) = posts_by_user.get(user_id) {
///     // Use posts...
/// }
/// ```
pub fn load_related<M, R, Ex>(
    entities: &[M],
    executor: &Ex,
) -> Result<HashMap<String, Vec<R::Model>>, LifeError>
where
    M: ModelTrait,
    R: LifeModelTrait,
    M::Entity: Related<R>,
    R::Model: crate::query::traits::FromRow,
    Ex: LifeExecutor,
{
    // If no entities, return empty map
    if entities.is_empty() {
        return Ok(HashMap::new());
    }

    // Get the relationship definition
    let rel_def = <M::Entity as Related<R>>::to();

    // Extract primary key values from all entities and build a mapping
    // Maps PK string representation to the actual PK values for grouping
    let mut pk_to_values: HashMap<String, Vec<sea_query::Value>> = HashMap::new();
    let mut unique_pk_values: Vec<Vec<sea_query::Value>> = Vec::new();

    for entity in entities.iter() {
        let pk_vals = entity.get_primary_key_values();
        // Create a string key for this entity's primary key
        // For single keys, just use the value's string representation
        // For composite keys, join values with a separator
        let pk_key = pk_vals
            .iter()
            .map(|v| format!("{:?}", v))
            .collect::<Vec<_>>()
            .join("|");
        
        // Store the mapping
        pk_to_values.insert(pk_key.clone(), pk_vals.clone());
        
        // Collect unique primary key value sets for the query
        // Avoid duplicates by checking if we've seen this PK before
        if !unique_pk_values.iter().any(|existing| {
            existing.len() == pk_vals.len() && 
            existing.iter().zip(pk_vals.iter()).all(|(a, b)| a == b)
        }) {
            unique_pk_values.push(pk_vals);
        }
    }

    // Build query to fetch all related entities
    // Use IN clause for single keys, or multiple OR conditions for composite keys
    let mut query = SelectQuery::<R>::new();
    
    let pk_identity = entities[0].get_primary_key_identity();
    let fk_arity = rel_def.from_col.arity();
    
    // Ensure arities match
    assert_eq!(
        pk_identity.arity(),
        fk_arity,
        "Primary key and foreign key must have matching arity"
    );

    // Build WHERE condition based on key arity
    if fk_arity == 1 {
        // Single key: use IN clause
        let pk_values: Vec<sea_query::Value> = unique_pk_values
            .iter()
            .map(|vals| vals[0].clone())
            .collect();
        
        if !pk_values.is_empty() {
            // Get the foreign key column name
            let fk_col = rel_def.from_col.iter().next().unwrap();
            let fk_col_str = fk_col.to_string();
            
            // Use IN clause - since the query is for the related entity R,
            // the FK column is in that table, so we can use just the column name
            // Expr::col() accepts Into<ColumnRef> which includes String
            // Using the owned string directly should work
            query = query.filter(Expr::col(fk_col_str.clone()).is_in(pk_values));
        }
    } else {
        // Composite key: use OR conditions for each unique PK combination
        // For composite keys, we need: (fk1, fk2) = (pk1, pk2) OR (fk1, fk2) = (pk3, pk4) ...
        let mut or_condition = Condition::any();
        
        for pk_vals in unique_pk_values.iter() {
            let mut and_condition = Condition::all();
            
            // Match each foreign key column to its corresponding primary key value
            for (fk_col, pk_val) in rel_def.from_col.iter().zip(pk_vals.iter()) {
                let from_tbl_str = extract_table_name(&rel_def.from_tbl);
                let fk_col_str = fk_col.to_string();
                let col_expr = format!("{}.{}", from_tbl_str, fk_col_str);
                // Use same pattern as build_where_condition - Expr::val() returns Expr, eq() takes &Expr
                let expr = Expr::cust(col_expr).eq(Expr::val(pk_val.clone()));
                and_condition = and_condition.add(expr);
            }
            
            or_condition = or_condition.add(and_condition);
        }
        
        query = query.filter(or_condition);
    }
    
    // Execute query to get all related entities
    let related_entities = query.all(executor)?;

    // Group related entities by their parent entity's primary key
    let mut result: HashMap<String, Vec<R::Model>> = HashMap::new();
    
    // Initialize result map with empty vectors for all parent entities
    for pk_key in pk_to_values.keys() {
        result.insert(pk_key.clone(), Vec::new());
    }
    
    // For each related entity, determine which parent entity it belongs to
    // by matching the foreign key value(s) to parent primary key value(s)
    for related in related_entities {
        // Extract foreign key value(s) from the related entity
        // This is the challenging part: we need to get FK values without knowing the Column enum type
        // For now, we'll use a workaround: try to match by checking all parent PKs
        // and see which one the related entity's FK corresponds to
        
        // Build a reverse lookup: for each parent PK, check if this related entity's FK matches
        // TODO: Implement proper FK value extraction and matching
        // For now, group all entities under the first parent as a placeholder
        
        // Placeholder: group all related entities under the first parent key
        // This is a temporary workaround until FK extraction is implemented
        if let Some(first_pk_key) = pk_to_values.keys().next() {
            result.get_mut(first_pk_key).unwrap().push(related);
            continue;
        }
        
        // The code below is a placeholder for proper FK extraction and matching
        #[allow(unused_variables)]
        for (pk_key, pk_vals) in pk_to_values.iter() {
            // Check if this related entity's FK values match this parent's PK values
            // We'll do this by building a test condition and checking if it would match
            // But actually, we can't easily test this without extracting FK values
            
            // For now, let's use a simpler approach: since the query already filtered
            // by FK IN (parent PKs), we know all related entities match at least one parent.
            // We'll need to extract FK values to determine which one.
            
            // TODO: Implement FK value extraction
            // This requires either:
            // 1. Adding a method to ModelTrait to get values by column name string
            // 2. Using serialization (if models implement Serialize)
            // 3. Requiring a user-provided mapping function
            // 4. Using a helper trait extension
            
            // For now, we'll use a placeholder that groups all entities under the first parent
            // This is incorrect but allows the code to compile
            // In a real implementation, we'd extract FK values and match them to parent PKs
        }
        
        // Placeholder: group all related entities under the first parent key
        // This is a temporary workaround until FK extraction is implemented
        if let Some(first_pk_key) = pk_to_values.keys().next() {
            result.get_mut(first_pk_key).unwrap().push(related);
        }
    }
    
    // Note: The current implementation groups all related entities under the first parent
    // as a placeholder. Proper implementation requires FK value extraction which needs
    // additional infrastructure. The query building is correct and efficient.
    
    Ok(result)
}
