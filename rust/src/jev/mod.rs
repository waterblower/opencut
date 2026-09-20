//! Async TypeSafe AI evaluation SDK.
//!
//! API contract: <https://docs.typesafe.ai/api>.
//! Credentials and runtime configuration are supplied by the caller.

pub use client::{Client, Config, RequestOptions};
pub use error::Error;
pub use types::{
    Content, JevAnswer, JevQuestion, NoulCriteria, Question, SystemOneRequest, SystemOneResponse,
    Usage,
};

mod client;
mod error;
mod types;

#[cfg(test)]
#[path = "tests/jev.test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/jev_success.test.rs"]
mod success_tests;
