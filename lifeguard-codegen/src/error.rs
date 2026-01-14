//! Error types for codegen

#[derive(Debug)]
pub enum CodegenError {
    Io(std::io::Error),
    Parse(String),
    Generation(String),
}

impl From<std::io::Error> for CodegenError {
    fn from(err: std::io::Error) -> Self {
        CodegenError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, CodegenError>;
