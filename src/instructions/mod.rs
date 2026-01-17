//! Instructions module for parsing and managing spec files
//!
//! Spec files are markdown documents that define tasks for Doodoori to execute.

#![allow(dead_code)]
#![allow(unused_imports)]

mod parser;
mod spec;
mod validation;

pub use parser::SpecParser;
pub use spec::{GlobalSettings, Requirement, SpecFile, TaskSpec};
pub use validation::{validate, ValidationError, ValidationResult};
