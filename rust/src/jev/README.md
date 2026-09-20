# Public interface proposal

This document defines the interface for review before client implementation.
Enable the `jev` feature and import public types from `opencut_player::jev`.
The API contract is <https://docs.typesafe.ai/api>.

## Client and configuration

```rust
// Clone shares the underlying HTTP connection pool. Fields are private.
pub struct Client { /* private fields */ }

impl Client {
    pub fn new(api_key: &str, config: ClientConfig) -> Result<Self, Error>;
    pub async fn system_one(
        &self,
        request: &SystemOneRequest,
        options: &RequestOptions,
    ) -> Result<SystemOneResponse, Error>;
}

pub struct ClientConfig {
    pub base_url: String,       // Default: https://api.typesafe.ai
    pub default_model: String,  // Default: jev-latest
    pub timeout: Duration,      // Default: 10 seconds per attempt
    pub max_retries: u32,       // Default: 2, after the initial attempt
}

pub struct RequestOptions {
    pub timeout: Option<Duration>,
    pub max_retries: Option<u32>,
}
```

Both configuration types implement `Clone`, `Debug`, and `Default`.
Unset request options inherit client settings; zero retries disables retries.
Construction validates credentials, URL, default model, and a positive timeout.
Credentials are copied into private client storage and excluded from debug output.
Calls borrow their inputs; one client supports concurrent requests on Tokio.
The SDK creates no runtime, reads no environment variables, and logs no errors.

## Requests and answers

Use the existing `Content`, `NoulCriteria`, `Question`, and `SystemOneRequest`
types in [types.rs](types.rs). `Content` supports text, objects, and arrays.
Question variants carry their own instructions and criteria. Named batches use
`BTreeMap<String, Question>`; `SystemOneRequest::new(state, questions)` leaves
`model` unset. Set `request.model = Some(model)` to override the client default.

```rust
pub enum Answer {
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
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
}

pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

Data types implement `Clone`, `Debug`, `PartialEq`, `Serialize`, and `Deserialize`.
Question and answer variants use explicit JSON `type` tags. Callers match answer
variants; the SDK preserves fractional scores, confidence, and probabilities.

## Errors and execution

```rust
pub enum Error {
    InvalidConfig(String),
    InvalidRequest(String),
    Transport(reqwest::Error),
    Timeout { timeout: Duration },
    Http { status: reqwest::StatusCode, body: Vec<u8> },
    Decode { source: serde_json::Error, body: Vec<u8> },
}
```

`Error` implements `Debug`, `Display`, and `std::error::Error`, retaining sources.
HTTP error bodies remain raw bytes because the error JSON schema is unspecified.
Validate requests before sending; malformed successful responses return `Decode`.
Each attempt's timeout includes body delivery. Retry only 429 and 529, waiting
500 ms initially, doubling to a 5-second cap. Dropping the future stops local
request processing and retry waits; it cannot undo work already received remotely.

## Interface tests

[tests/jev.test.rs](tests/jev.test.rs) demonstrates mixed batches, matching typed
answers, configuration overrides, async calls, and matching validation errors.
These tests intentionally reference public types awaiting implementation.
Run them with `cargo test --no-default-features --features jev --lib jev::tests`.
