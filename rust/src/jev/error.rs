use reqwest::{Error as TransportError, StatusCode};
use serde_json::Error as DecodeError;
use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

/// Failures returned to the caller without logging or UI reporting.
#[derive(Debug)]
pub enum Error {
    InvalidConfig(String),
    InvalidRequest(String),
    InvalidResponse(String),
    Transport(TransportError),
    Timeout { timeout: Duration },
    Http { status: StatusCode, body: Vec<u8> },
    Decode { source: DecodeError, body: Vec<u8> },
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::InvalidConfig(message) => write!(formatter, "invalid configuration: {message}"),
            Self::InvalidRequest(message) => write!(formatter, "invalid request: {message}"),
            Self::InvalidResponse(message) => write!(formatter, "invalid response: {message}"),
            Self::Transport(source) => write!(formatter, "request transport failed: {source}"),
            Self::Timeout { timeout } => write!(formatter, "request timed out after {timeout:?}"),
            Self::Http { status, .. } => write!(formatter, "API returned HTTP {status}"),
            Self::Decode { source, .. } => {
                write!(formatter, "could not decode API response: {source}")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(source) => Some(source),
            Self::Decode { source, .. } => Some(source),
            _ => None,
        }
    }
}
