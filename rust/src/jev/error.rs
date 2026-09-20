use reqwest::{Error as TransportError, StatusCode};
use serde_json::Error as DecodeError;
use std::time::Duration;

/// Failures returned to the caller without logging or UI reporting.
#[derive(Debug)]
pub enum Error {
    InvalidConfig(String),
    InvalidRequest(String),
    Transport(TransportError),
    Timeout { timeout: Duration },
    Http { status: StatusCode, body: Vec<u8> },
    Decode { source: DecodeError, body: Vec<u8> },
}
