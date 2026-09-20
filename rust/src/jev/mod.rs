//! Async TypeSafe AI evaluation SDK.
//!
//! API contract: <https://docs.typesafe.ai/api>.
//! Credentials and runtime configuration are supplied by the caller.
//!
//! Enable the `jev` Cargo feature and call the client inside a Tokio runtime.
//! Requests have a fixed five-second timeout and are never retried.
//!
//! ```no_run
//! use opencut_player::jev::{Client, Config, Question, SystemOneRequest};
//! use std::{collections::BTreeMap, env, error::Error as StdError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let client = Client::new(Config {
//!         api_key: env::var("TYPESAFE_API_KEY")?,
//!         ..Config::default()
//!     })?;
//!     let request = SystemOneRequest {
//!         state: "I was charged twice for the same order.".into(),
//!         questions: BTreeMap::from([("billing".into(), Question::Noul {
//!             instructions: "Is this about billing?".into(),
//!             criteria: None,
//!         })]),
//!         model: None,
//!     };
//!     let response = client.send(&request).await?;
//!     println!("{:?}", response.answers["billing"]);
//!     Ok(())
//! }
//! ```

pub use client::{Client, Config};
pub use error::Error;
pub use types::{
    Content, JevAnswer, NoulCriteria, Question, SystemOneRequest, SystemOneResponse, Usage,
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
