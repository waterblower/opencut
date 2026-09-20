use crate::jev::{Error, JevAnswer, JevQuestion, Question, SystemOneRequest, SystemOneResponse};
use reqwest::{Client as HttpClient, Url, header::HeaderValue, redirect::Policy, retry::never};
use std::{fmt, time::Duration};

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
        _question: JevQuestion,
        _options: &RequestOptions,
    ) -> Result<JevAnswer, Error> {
        unimplemented!("convert a single question, send its batch, and extract its answer")
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
        unimplemented!("serialize, send with timeout and retries, then decode")
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
