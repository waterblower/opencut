use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Text or structured JSON accepted as state, instructions, and descriptions.
/// Nested values may contain any JSON value, including null and numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Object(Map<String, Value>),
    Array(Vec<Value>),
}

/// Optional descriptions of the two outcomes of a yes/no question.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoulCriteria {
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub yes: Option<Content>,
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub no: Option<Content>,
}

/// A question in a named evaluation batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Question {
    /// Estimate the probability that the answer is yes.
    #[serde(rename = "noul")]
    Noul {
        instructions: Content,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Choose a label from at most 255 alternatives.
    #[serde(rename = "choice")]
    Choice {
        instructions: Content,
        /// A null description leaves the corresponding label undescribed.
        criteria: BTreeMap<String, Option<Content>>,
    },
    /// Evaluate an ordered rubric containing 2–10 levels.
    #[serde(rename = "score")]
    Score {
        instructions: Content,
        /// Level indices start at zero and follow this vector's order.
        criteria: Vec<Content>,
    },
}

/// Input to a System One evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemOneRequest {
    pub state: Content,
    /// Nonempty map; answers are returned under these same names.
    pub questions: BTreeMap<String, Question>,
    /// None selects the client's default model when sending the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl SystemOneRequest {
    /// Creates an evaluation using the client's default model.
    pub fn new(state: Content, questions: BTreeMap<String, Question>) -> Self {
        Self {
            state,
            questions,
            model: None,
        }
    }
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}
