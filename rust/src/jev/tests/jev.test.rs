use crate::jev::{Client, Config, Error, JevAnswer, Question, SystemOneRequest, SystemOneResponse};
use serde_json::json;
use std::{collections::BTreeMap, io::ErrorKind, net::TcpListener};

#[test]
fn builds_a_named_batch_with_all_three_question_types() {
    let request = SystemOneRequest {
        state: "My payouts have been failing for three days!".into(),
        questions: BTreeMap::from([
            (
                "urgent".into(),
                Question::Noul {
                    instructions: "Does this convey urgency?".into(),
                    criteria: None,
                },
            ),
            (
                "department".into(),
                Question::Choice {
                    instructions: "Which team should handle this?".into(),
                    criteria: BTreeMap::from([
                        ("billing".into(), Some("Payments and refunds".into())),
                        ("other".into(), None),
                    ]),
                },
            ),
            (
                "frustration".into(),
                Question::Score {
                    instructions: "How frustrated is the customer?".into(),
                    criteria: vec!["Calm".into(), "Frustrated".into(), "Very angry".into()],
                },
            ),
        ]),
        model: None,
    };
    assert!(request.model.is_none()); // The client selects jev-latest when sending.
    let body = serde_json::to_value(&request).unwrap();
    assert_eq!(
        body["state"],
        "My payouts have been failing for three days!"
    );
    assert_eq!(
        body["questions"],
        json!({
            "urgent": {"type": "noul", "instructions": "Does this convey urgency?"},
            "department": {
                "type": "choice", "instructions": "Which team should handle this?",
                "criteria": {"billing": "Payments and refunds", "other": null}
            },
            "frustration": {
                "type": "score", "instructions": "How frustrated is the customer?",
                "criteria": ["Calm", "Frustrated", "Very angry"]
            }
        })
    );
}

#[test]
fn reads_typed_answers_and_usage_from_a_response() {
    let response: SystemOneResponse = serde_json::from_value(json!({
        "model": "jev-1.13.0",
        "answers": {
            "urgent": {"type": "noul", "noul": 0.95},
            "department": {
                "type": "choice", "choice": "billing", "confidence": 0.81,
                "probabilities": {"billing": 0.88, "other": 0.12}
            },
            "frustration": {
                "type": "score", "score": 1.05, "confidence": 0.92,
                "legend": {"0": "Calm", "1": "Frustrated", "2": "Very angry"},
                "probabilities": {"0": 0.0, "1": 0.95, "2": 0.05}
            }
        },
        "usage": {"input_tokens": 304, "output_tokens": 18}
    }))
    .unwrap();
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.usage.input_tokens, 304);
    assert_eq!(response.usage.output_tokens, 18);
    let JevAnswer::Noul { noul } = &response.answers["urgent"] else {
        panic!("expected a yes/no answer");
    };
    assert_eq!(*noul, 0.95);
    let JevAnswer::Choice {
        choice,
        probabilities,
        ..
    } = &response.answers["department"]
    else {
        panic!("expected a choice answer");
    };
    assert_eq!(choice, "billing");
    assert_eq!(probabilities["billing"], 0.88);
    let JevAnswer::Score { score, legend, .. } = &response.answers["frustration"] else {
        panic!("expected a score answer");
    };
    assert_eq!(*score, 1.05);
    assert_eq!(legend["1"], "Frustrated");
}

#[tokio::test]
async fn rejects_an_empty_batch_before_sending() {
    let client = Client::new(Config {
        api_key: "test-key".into(),
        base_url: "http://127.0.0.1:1".into(),
        ..Config::default()
    })
    .unwrap();
    let request = SystemOneRequest {
        state: "A customer message".into(),
        questions: BTreeMap::new(),
        model: None,
    };
    let error = client.send(&request).await.unwrap_err();
    assert!(matches!(error, Error::InvalidRequest(_)));
}

#[test]
fn configuration_defaults_to_the_typesafe_endpoint() {
    let config = Config::default();
    assert!(config.api_key.is_empty());
    assert_eq!(config.base_url, "https://api.typesafe.ai");
}

#[test]
fn rejects_invalid_client_configuration() {
    for api_key in ["", "   ", "test\nkey"] {
        let result = Client::new(Config {
            api_key: api_key.into(),
            ..Config::default()
        });
        assert!(matches!(result, Err(Error::InvalidConfig(_))));
    }
    for base_url in ["not a URL", "ftp://api.typesafe.ai"] {
        let config = Config {
            api_key: "test-key".into(),
            base_url: base_url.into(),
        };
        assert!(matches!(Client::new(config), Err(Error::InvalidConfig(_))));
    }
}

#[test]
fn preserves_structured_content_optional_criteria_and_model_override() {
    let body = json!({
        "state": {"messages": [{"text": "Help!", "priority": 2, "extra": null}]},
        "model": "jev-1.13.0",
        "questions": {
            "urgent": {
                "type": "noul",
                "instructions": ["Does this convey urgency?", {"consider": "priority"}],
                "criteria": {"true": {"description": "Time-sensitive"}}
            }
        }
    });
    let request: SystemOneRequest = serde_json::from_value(body.clone()).unwrap();
    let Question::Noul { criteria, .. } = &request.questions["urgent"] else {
        panic!("expected a yes/no question");
    };
    let criteria = criteria.as_ref().unwrap();
    assert!(criteria.yes.is_some());
    assert!(criteria.no.is_none());
    assert_eq!(request.model.as_deref(), Some("jev-1.13.0"));
    assert_eq!(serde_json::to_value(request).unwrap(), body);
}

#[tokio::test]
async fn rejects_invalid_criteria_without_contacting_the_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let client = Client::new(Config {
        api_key: "test-key".into(),
        base_url: format!("http://{}", listener.local_addr().unwrap()),
    })
    .unwrap();
    let mut invalid_questions = Vec::new();
    for count in [0, 1, 11] {
        invalid_questions.push(Question::Score {
            instructions: "Rate urgency".into(),
            criteria: vec!["Level".into(); count],
        });
    }
    for count in [0, 256] {
        let mut criteria = BTreeMap::new();
        for index in 0..count {
            criteria.insert(index.to_string(), None);
        }
        invalid_questions.push(Question::Choice {
            instructions: "Choose a department".into(),
            criteria,
        });
    }
    for question in invalid_questions {
        let request = SystemOneRequest {
            state: "Help!".into(),
            questions: BTreeMap::from([("result".into(), question)]),
            model: None,
        };
        let result = client.send(&request).await;
        assert!(matches!(result, Err(Error::InvalidRequest(_))));
    }
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[test]
fn configuration_debug_output_redacts_the_api_key() {
    let config = Config {
        api_key: "secret-test-credential".into(),
        ..Config::default()
    };
    for debug in [format!("{config:?}"), format!("{config:#?}")] {
        assert!(!debug.contains(&config.api_key));
        assert!(debug.contains("[redacted]"));
    }
}
