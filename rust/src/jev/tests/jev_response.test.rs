use crate::jev::{Error, JevAnswer, Question, client::decode_response};
use serde_json::json;
use std::{collections::BTreeMap, error::Error as StdError};

#[test]
fn matches_answers_to_requested_names_and_types() {
    let questions = BTreeMap::from([(
        "urgent".into(),
        Question::Noul {
            instructions: "Is this urgent?".into(),
            criteria: None,
        },
    )]);
    for (answers, valid) in [
        (json!({"urgent": {"type": "noul", "noul": 0.95}}), true),
        (json!({}), false),
        (json!({"wrong_name": {"type": "noul", "noul": 0.95}}), false),
        (
            json!({"urgent": {"type": "noul", "noul": 0.95},
            "extra": {"type": "noul", "noul": 0.1}}),
            false,
        ),
        (
            json!({"urgent": {"type": "choice", "choice": "yes",
            "probabilities": {"yes": 1.0}, "confidence": 1.0}}),
            false,
        ),
    ] {
        let body = serde_json::to_vec(&json!({
            "model": "jev-1.13.0", "answers": answers,
            "usage": {"input_tokens": 304, "output_tokens": 18},
        }))
        .unwrap();
        let result = decode_response(&body, &questions);
        if valid {
            let response = result.unwrap();
            assert_eq!(response.answers["urgent"], JevAnswer::Noul { noul: 0.95 });
            assert_eq!(response.model, "jev-1.13.0");
            assert_eq!(response.usage.input_tokens, 304);
            assert_eq!(response.usage.output_tokens, 18);
        } else {
            assert!(matches!(result, Err(Error::InvalidResponse(_))));
        }
    }
}

#[test]
fn malformed_responses_retain_the_body_and_error_source() {
    for body in [b"not JSON".as_slice(), b"{}".as_slice()] {
        let error = decode_response(body, &BTreeMap::new()).unwrap_err();
        assert!(error.source().is_some());
        let Error::Decode { body: received, .. } = error else {
            panic!("expected a decoding error");
        };
        assert_eq!(received, body);
    }
}
