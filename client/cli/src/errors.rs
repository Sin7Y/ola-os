use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid file: {0}")]
    InvalidFilePath(String),
    #[error("IO error: {0}")]
    IOError(io::Error),
    #[error("failed to parse LLVM IR")]
    IRParseFailed,
    #[error("compile module failed")]
    CompileModuleFailed,
    #[error("namespace parse error")]
    CompileNameSpaceError,
    #[error("asm complie failed: {0}")]
    AsmCompileFailed(String),
}
