//! Live API tests. Set TYPESAFE_API_KEY and explicitly run with --ignored.
use crate::jev::{Client, Config, JevAnswer, Question, SystemOneRequest};
use std::{collections::BTreeMap, env};

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_noul_returns_a_probability() {
    let client = initialize_client();
    let request = SystemOneRequest {
        state: "My account was charged twice for the same order.".into(),
        questions: BTreeMap::from([(
            "billing".into(),
            Question::Noul {
                instructions: "Is this about billing?".into(),
                criteria: None,
            },
        )]),
        model: None,
    };
    let response = client.send(&request).await.unwrap();
    let JevAnswer::Noul { noul } = &response.answers["billing"] else {
        panic!("expected a yes/no answer");
    };
    assert!((0.0..=1.0).contains(noul));
    assert!(!response.model.is_empty());
    assert!(response.usage.input_tokens > 0);
}

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_choice_returns_a_label_and_distribution() {
    let client = initialize_client();
    let request = SystemOneRequest {
        state: "Please refund the duplicate charge on my invoice.".into(),
        questions: BTreeMap::from([(
            "department".into(),
            Question::Choice {
                instructions: "Which team should handle this?".into(),
                criteria: BTreeMap::from([
                    ("billing".into(), Some("Payments and refunds".into())),
                    ("technical".into(), Some("Software errors".into())),
                    ("other".into(), None),
                ]),
            },
        )]),
        model: None,
    };
    let response = client.send(&request).await.unwrap();
    let JevAnswer::Choice {
        choice,
        probabilities,
        confidence,
    } = &response.answers["department"]
    else {
        panic!("expected a choice answer");
    };
    assert!(probabilities.contains_key(choice));
    assert_eq!(
        probabilities.keys().collect::<Vec<_>>(),
        ["billing", "other", "technical"]
    );
    assert!((0.0..=1.0).contains(confidence));
    assert_probabilities(probabilities);
}

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_score_returns_a_score_and_legend() {
    let client = initialize_client();
    let request = SystemOneRequest {
        state: "The payment system is down. Please fix it immediately!".into(),
        questions: BTreeMap::from([(
            "urgency".into(),
            Question::Score {
                instructions: "How urgent is this request?".into(),
                criteria: vec![
                    "No urgency".into(),
                    "Some urgency".into(),
                    "Very urgent".into(),
                ],
            },
        )]),
        model: None,
    };
    let response = client.send(&request).await.unwrap();
    let JevAnswer::Score {
        score,
        legend,
        probabilities,
        confidence,
    } = &response.answers["urgency"]
    else {
        panic!("expected a score answer");
    };
    assert!((0.0..=2.0).contains(score));
    assert!((0.0..=1.0).contains(confidence));
    assert_eq!(
        legend,
        &BTreeMap::from([
            ("0".into(), "No urgency".into()),
            ("1".into(), "Some urgency".into()),
            ("2".into(), "Very urgent".into()),
        ])
    );
    assert_eq!(probabilities.keys().collect::<Vec<_>>(), ["0", "1", "2"]);
    assert_probabilities(probabilities);
}

fn initialize_client() -> Client {
    let api_key = env::var("TYPESAFE_API_KEY").expect("set TYPESAFE_API_KEY to run live tests");
    Client::new(Config {
        api_key,
        ..Config::default()
    })
    .unwrap()
}

fn assert_probabilities(probabilities: &BTreeMap<String, f64>) {
    for probability in probabilities.values() {
        assert!((0.0..=1.0).contains(probability));
    }
    assert!((probabilities.values().sum::<f64>() - 1.0).abs() < 0.001);
}
