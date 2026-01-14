//! Lifeguard Codegen Library
//!
//! This library provides code generation functionality for Lifeguard ORM entities.
//! The main entry point is the `EntityWriter` which generates Entity, Model, Column, etc.

pub mod entity;
pub mod error;
pub mod parser;
pub mod writer;

pub use entity::{EntityDefinition, FieldDefinition};
pub use writer::EntityWriter;
