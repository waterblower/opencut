//! Live API tests. Set TYPESAFE_API_KEY and explicitly run with --ignored.
use crate::jev::{Client, Config, JevAnswer, JevQuestion};
use std::{collections::BTreeMap, env};

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_noul_returns_a_probability() {
    let client = initialize_client();
    let answer = client
        .send(JevQuestion::Noul {
            state: "My account was charged twice for the same order.".into(),
            question: "Is this about billing?".into(),
            criteria: None,
        })
        .await
        .unwrap();
    let JevAnswer::Noul { noul } = answer else {
        panic!("expected a yes/no answer");
    };
    assert!((0.0..=1.0).contains(&noul));
}

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_choice_returns_a_label_and_distribution() {
    let client = initialize_client();
    let answer = client
        .send(JevQuestion::Choice {
            state: "Please refund the duplicate charge on my invoice.".into(),
            question: "Which team should handle this?".into(),
            criteria: vec![
                ("billing".into(), Some("Payments and refunds".into())),
                ("technical".into(), Some("Software errors".into())),
                ("other".into(), None),
            ],
        })
        .await
        .unwrap();
    let JevAnswer::Choice {
        choice,
        probabilities,
        confidence,
    } = answer
    else {
        panic!("expected a choice answer");
    };
    assert!(probabilities.contains_key(&choice));
    assert_eq!(
        probabilities.keys().collect::<Vec<_>>(),
        ["billing", "other", "technical"]
    );
    assert!((0.0..=1.0).contains(&confidence));
    assert_probabilities(&probabilities);
}

#[tokio::test]
#[ignore = "calls the live TypeSafe API; requires TYPESAFE_API_KEY"]
async fn live_score_returns_a_score_and_legend() {
    let client = initialize_client();
    let answer = client
        .send(JevQuestion::Score {
            state: "The payment system is down. Please fix it immediately!".into(),
            question: "How urgent is this request?".into(),
            criteria: vec![
                "No urgency".into(),
                "Some urgency".into(),
                "Very urgent".into(),
            ],
        })
        .await
        .unwrap();
    let JevAnswer::Score {
        score,
        legend,
        probabilities,
        confidence,
    } = answer
    else {
        panic!("expected a score answer");
    };
    assert!((0.0..=2.0).contains(&score));
    assert!((0.0..=1.0).contains(&confidence));
    assert_eq!(
        legend,
        BTreeMap::from([
            ("0".into(), "No urgency".into()),
            ("1".into(), "Some urgency".into()),
            ("2".into(), "Very urgent".into()),
        ])
    );
    assert_eq!(probabilities.keys().collect::<Vec<_>>(), ["0", "1", "2"]);
    assert_probabilities(&probabilities);
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
