use crate::jev::{Content, Error, JevAnswer, Question, SystemOneRequest, SystemOneResponse};
use reqwest::{
    Client as HttpClient, Error as TransportError, Url,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
    redirect::Policy,
    retry::never,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter, Result as FmtResult},
    time::Duration,
};
use tokio::time::timeout;

/// Reusable client. Clones share the HTTP connection pool.
#[derive(Clone)]
pub struct Client {
    http: HttpClient,
    authorization: HeaderValue,
    config: Config,
}

#[derive(Clone)]
pub struct Config {
    /// Required credential; empty by default and redacted from debug output.
    pub api_key: String,
    pub base_url: String,
}

impl Client {
    /// Validates configuration and creates a reusable HTTP connection pool.
    pub fn new(mut config: Config) -> Result<Self, Error> {
        if config.api_key.trim().is_empty() {
            return Err(Error::InvalidConfig("an API key is required".into()));
        }
        let mut authorization = match HeaderValue::from_str(&format!("Bearer {}", config.api_key)) {
            Ok(value) => value,
            Err(_) => return Err(Error::InvalidConfig("invalid API key header".into())),
        };
        authorization.set_sensitive(true);
        let url = match Url::parse(&config.base_url) {
            Ok(url) => url,
            Err(_) => return Err(Error::InvalidConfig("invalid API base URL".into())),
        };
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::InvalidConfig(
                "API base URL must be HTTP(S) without credentials, query, or fragment".into(),
            ));
        }
        config.base_url = url.as_str().trim_end_matches('/').to_owned();
        let http = match HttpClient::builder()
            .retry(never())
            .redirect(Policy::none())
            .build()
        {
            Ok(http) => http,
            Err(error) => return Err(Error::Transport(error)),
        };
        Ok(Self {
            http,
            authorization,
            config,
        })
    }

    /// Evaluates named questions, preserving the model and token usage.
    /// Makes one attempt with a five-second timeout including response body delivery.
    /// Returns `InvalidResponse` if answer names or types do not match the request.
    pub async fn send(&self, request: &SystemOneRequest) -> Result<SystemOneResponse, Error> {
        let model = request.model.as_deref().unwrap_or("jev-latest");
        if model.trim().is_empty() {
            return Err(Error::InvalidRequest("a model is required".into()));
        }
        if request.questions.is_empty() {
            return Err(Error::InvalidRequest(
                "at least one question is required".into(),
            ));
        }
        for (name, question) in &request.questions {
            match question {
                Question::Choice { criteria, .. } if !(1..=255).contains(&criteria.len()) => {
                    return Err(Error::InvalidRequest(format!(
                        "question {name:?} requires 1–255 choice labels"
                    )));
                }
                Question::Score { criteria, .. } if !(2..=10).contains(&criteria.len()) => {
                    return Err(Error::InvalidRequest(format!(
                        "question {name:?} requires 2–10 score levels"
                    )));
                }
                _ => {}
            }
        }
        let body = match serde_json::to_vec(&RequestBody {
            state: &request.state,
            questions: &request.questions,
            model,
        }) {
            Ok(body) => body,
            Err(error) => {
                return Err(Error::InvalidRequest(format!(
                    "could not encode request: {error}"
                )));
            }
        };
        let url = format!("{}/v1/systemone", self.config.base_url);
        let attempt = async {
            let response = self
                .http
                .post(&url)
                .header(AUTHORIZATION, self.authorization.clone())
                .header(ACCEPT, "application/json")
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await?;
            let status = response.status();
            let bytes = response.bytes().await?;
            Ok::<_, TransportError>((status, bytes))
        };
        let (status, bytes) = match timeout(REQUEST_TIMEOUT, attempt).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) if error.is_timeout() => {
                return Err(Error::Timeout {
                    timeout: REQUEST_TIMEOUT,
                });
            }
            Ok(Err(error)) => return Err(Error::Transport(error)),
            Err(_) => {
                return Err(Error::Timeout {
                    timeout: REQUEST_TIMEOUT,
                });
            }
        };
        if !status.is_success() {
            return Err(Error::Http {
                status,
                body: bytes.to_vec(),
            });
        }
        decode_response(&bytes, &request.questions)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.typesafe.ai".into(),
        }
    }
}

impl Debug for Config {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("Config")
            .field("api_key", &"[redacted]")
            .field("base_url", &self.base_url)
            .finish()
    }
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize)]
struct RequestBody<'a> {
    state: &'a Content,
    questions: &'a BTreeMap<String, Question>,
    model: &'a str,
}

fn decode_response(
    body: &[u8],
    questions: &BTreeMap<String, Question>,
) -> Result<SystemOneResponse, Error> {
    let response: SystemOneResponse = match serde_json::from_slice(body) {
        Ok(response) => response,
        Err(source) => {
            return Err(Error::Decode {
                source,
                body: body.to_vec(),
            });
        }
    };
    if response.answers.len() != questions.len() {
        return Err(Error::InvalidResponse(
            "answer count does not match the request".into(),
        ));
    }
    for (name, question) in questions {
        let Some(answer) = response.answers.get(name) else {
            return Err(Error::InvalidResponse(format!(
                "missing answer for question {name:?}"
            )));
        };
        if !matches!(
            (question, answer),
            (Question::Noul { .. }, JevAnswer::Noul { .. })
                | (Question::Choice { .. }, JevAnswer::Choice { .. })
                | (Question::Score { .. }, JevAnswer::Score { .. })
        ) {
            return Err(Error::InvalidResponse(format!(
                "answer type does not match question {name:?}"
            )));
        }
    }
    Ok(response)
}

#[cfg(test)]
#[path = "tests/jev_response.test.rs"]
mod response_tests;
