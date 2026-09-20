//! Async TypeSafe AI evaluation SDK.
//!
//! API contract: <https://docs.typesafe.ai/api>.
//! Credentials and runtime configuration are supplied by the caller.
//!
//! Enable the `jev` Cargo feature and call the client inside a Tokio runtime.
//! Dropping a request future cancels local work and pending retries.
//!
//! ```no_run
//! use opencut_player::jev::{Client, Config, Question, RequestOptions, SystemOneRequest};
//! use std::{collections::BTreeMap, env, error::Error as StdError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let client = Client::new(Config {
//!         api_key: env::var("TYPESAFE_API_KEY")?,
//!         ..Config::default()
//!     })?;
//!     let request = SystemOneRequest::new(
//!         "I was charged twice for the same order.".into(),
//!         BTreeMap::from([("billing".into(), Question::Noul {
//!             instructions: "Is this about billing?".into(),
//!             criteria: None,
//!         })]),
//!     );
//!     let response = client.send(&request, &RequestOptions::default()).await?;
//!     println!("{:?}", response.answers["billing"]);
//!     Ok(())
//! }
//! ```

pub use client::{Client, Config, RequestOptions};
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
