//! Async TypeSafe AI evaluation SDK.
//!
//! API contract: <https://docs.typesafe.ai/api>.
//! Credentials and runtime configuration are supplied by the caller.

mod types;

pub use types::{Content, NoulCriteria, Question, SystemOneRequest};

#[cfg(test)]
#[path = "tests/jev.test.rs"]
mod tests;
