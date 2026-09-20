# TypeSafe AI SDK

Async Tokio SDK for single questions and named batches, using reqwest with Rustls.
Enable the `jev` feature and import public types from `opencut_player::jev`.
The API contract is <https://docs.typesafe.ai/api>.

## Example

Set `TYPESAFE_API_KEY` in your environment and enable the `jev` Cargo feature.
Read the key during application initialization, then pass it to the client:

```rust
use opencut_player::jev::{Client, Config, Question, SystemOneRequest};
use std::{collections::BTreeMap, env, error::Error as StdError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let client = Client::new(Config {
        api_key: env::var("TYPESAFE_API_KEY")?,
        ..Config::default()
    })?;

    let request = SystemOneRequest {
        state: "I was charged twice for the same order.".into(),
        questions: BTreeMap::from([(
            "billing".into(),
            Question::Noul {
                instructions: "Is this about billing?".into(),
                criteria: None,
            },
        )]),
        model: None,
    };

    let response = client.send(&request).await?;
    println!("{:?}", response.answers["billing"]);
    println!("Model: {}", response.model);
    println!("Input tokens: {}", response.usage.input_tokens);
    Ok(())
}
```

Add more named questions to the map to evaluate them against the same state in
one request. The client uses `jev-latest`, a five-second timeout, and no retries.

## Client and configuration

```rust
// Clone shares the underlying HTTP connection pool. Fields are private.
pub struct Client { /* private fields */ }

impl Client {
    pub fn new(config: Config) -> Result<Self, Error>;
    pub async fn send(
        &self,
        request: &SystemOneRequest,
    ) -> Result<SystemOneResponse, Error>;
}

pub struct Config {
    pub api_key: String,        // Required; empty by default
    pub base_url: String,       // Default: https://api.typesafe.ai
}
```

`Config` implements `Clone`, `Debug`, and `Default`.
Construction validates credentials and the URL.
Credentials are copied into private client storage and excluded from debug output.
Calls borrow their request.
One client supports concurrent requests on Tokio.
The SDK creates no runtime, reads no environment variables, and logs no errors.

## Requests and answers

Use `send()` for one or more named questions against shared state.
It returns named answers together with the model and token usage.
Use `Content`, `NoulCriteria`, `Question`, and `SystemOneRequest`
types in [types.rs](types.rs). `Content` supports text, objects, and arrays.
Question variants carry their own instructions and criteria. Named batches use
`BTreeMap<String, Question>`. Construct `SystemOneRequest` with a struct literal;
set `model: None` to select `jev-latest`, or `model: Some(model)` to pin
a version or select another alias for that batch.

```rust
pub enum JevAnswer {
    Noul { noul: f64 },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Score {
        score: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}

pub struct SystemOneResponse {
    pub model: String,
    pub answers: BTreeMap<String, JevAnswer>,
    pub usage: Usage,
}

pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

Data types implement `Clone`, `Debug`, and `PartialEq`; wire types also implement
`Serialize` and `Deserialize`.
Question and answer variants use explicit JSON `type` tags. Callers match answer
variants; the SDK preserves fractional scores, confidence, and probabilities.

## Errors and execution

```rust
pub enum Error {
    InvalidConfig(String),
    InvalidRequest(String),
    InvalidResponse(String),
    Transport(reqwest::Error),
    Timeout { timeout: Duration },
    Http { status: reqwest::StatusCode, body: Vec<u8> },
    Decode { source: serde_json::Error, body: Vec<u8> },
}
```

`Error` implements `Debug`, `Display`, and `std::error::Error`, retaining sources.
HTTP error bodies remain raw bytes because the error JSON schema is unspecified.
Validate requests before sending; malformed successful responses return `Decode`.
Missing, extra, misnamed, or incorrectly typed answers return `InvalidResponse`.
Every request has a fixed five-second timeout covering connection and response
body delivery. No failures are retried, including HTTP 429 and 529. Dropping the
future stops local request processing; it cannot undo work already received remotely.

## Validation

[tests/jev.test.rs](tests/jev.test.rs) demonstrates mixed batches, matching typed
answers, configuration, async calls, and matching validation errors.
[tests/jev_response.test.rs](tests/jev_response.test.rs) checks response validation
and decoding errors using JSON fixtures. These tests never call TypeSafe.
One validation test binds a loopback socket to verify that invalid criteria cause
no connection; it does not serve mock responses.

Run from the Rust project directory:

```sh
cargo check --no-default-features --features jev --lib
cargo test --no-default-features --features jev --lib jev::
cargo test --no-default-features --features jev --doc
```

[tests/jev_success.test.rs](tests/jev_success.test.rs) contains live success cases
for Noul, Choice, and Score. They are ignored by default and read credentials only
during test initialization. Set `TYPESAFE_API_KEY`, then explicitly run:

```sh
cargo test --no-default-features --features jev --lib jev::success_tests -- --ignored
```

Live tests incur normal API usage. Timeout and cancellation behavior has
not been verified against the live service; no mock HTTP server is used.
