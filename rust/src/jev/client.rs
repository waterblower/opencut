use crate::jev::{
    Content, Error, JevAnswer, JevQuestion, Question, SystemOneRequest, SystemOneResponse,
};
use reqwest::{
    Client as HttpClient, Error as TransportError, Url,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
    redirect::Policy,
    retry::never,
};
use serde::Serialize;
use std::{collections::BTreeMap, fmt, time::Duration};
use tokio::time::{Instant, sleep, timeout_at};

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
    /// Timeout for each attempt, including response body delivery.
    pub timeout: Duration,
    /// Number of retries after the initial attempt; zero disables retries.
    pub max_retries: u32,
}

/// Unset options inherit the corresponding client settings.
#[derive(Clone, Debug, Default)]
pub struct RequestOptions {
    pub timeout: Option<Duration>,
    pub max_retries: Option<u32>,
}

impl Client {
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
        if config.timeout.is_zero() {
            return Err(Error::InvalidConfig("timeout must be positive".into()));
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

    pub async fn send(&self, question: JevQuestion) -> Result<JevAnswer, Error> {
        self.send_with_options(question, &RequestOptions::default())
            .await
    }

    pub async fn send_with_options(
        &self,
        question: JevQuestion,
        options: &RequestOptions,
    ) -> Result<JevAnswer, Error> {
        let (state, question) = match question {
            JevQuestion::Noul {
                state,
                question,
                criteria,
            } => (
                state,
                Question::Noul {
                    instructions: question,
                    criteria,
                },
            ),
            JevQuestion::Choice {
                state,
                question,
                criteria,
            } => {
                let mut labels = BTreeMap::new();
                for (label, description) in criteria {
                    if labels.insert(label.clone(), description).is_some() {
                        return Err(Error::InvalidRequest(format!(
                            "duplicate choice label {label:?}"
                        )));
                    }
                }
                (
                    state,
                    Question::Choice {
                        instructions: question,
                        criteria: labels,
                    },
                )
            }
            JevQuestion::Score {
                state,
                question,
                criteria,
            } => (
                state,
                Question::Score {
                    instructions: question,
                    criteria,
                },
            ),
        };
        let request = SystemOneRequest::new(state, BTreeMap::from([("result".into(), question)]));
        let mut response = self.send_batch(&request, options).await?;
        let Some(answer) = response.answers.remove("result") else {
            return Err(Error::InvalidResponse(
                "missing answer for the question".into(),
            ));
        };
        Ok(answer)
    }

    pub async fn send_batch(
        &self,
        request: &SystemOneRequest,
        options: &RequestOptions,
    ) -> Result<SystemOneResponse, Error> {
        let timeout = options.timeout.unwrap_or(self.config.timeout);
        if timeout.is_zero() {
            return Err(Error::InvalidRequest("timeout must be positive".into()));
        }
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
        let max_retries = options.max_retries.unwrap_or(self.config.max_retries);
        let mut retries = 0;
        let mut backoff = Duration::from_millis(500);
        loop {
            let Some(deadline) = Instant::now().checked_add(timeout) else {
                return Err(Error::InvalidRequest("timeout is too large".into()));
            };
            let attempt = async {
                let response = self
                    .http
                    .post(&url)
                    .header(AUTHORIZATION, self.authorization.clone())
                    .header(ACCEPT, "application/json")
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
                    .send()
                    .await?;
                let status = response.status();
                let bytes = response.bytes().await?;
                Ok::<_, TransportError>((status, bytes))
            };
            let (status, bytes) = match timeout_at(deadline, attempt).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) if error.is_timeout() => return Err(Error::Timeout { timeout }),
                Ok(Err(error)) => return Err(Error::Transport(error)),
                Err(_) => return Err(Error::Timeout { timeout }),
            };
            if status.is_success() {
                return decode_response(&bytes, &request.questions);
            }
            if !matches!(status.as_u16(), 429 | 529) || retries == max_retries {
                return Err(Error::Http {
                    status,
                    body: bytes.to_vec(),
                });
            }
            retries += 1;
            sleep(backoff).await;
            backoff = backoff.saturating_mul(2).min(Duration::from_secs(5));
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.typesafe.ai".into(),
            timeout: Duration::from_secs(10),
            max_retries: 2,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("api_key", &"[redacted]")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

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
