use super::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
};

#[tokio::test]
async fn sends_documented_multipart_fields_and_preserves_provider_json() {
    let expected = json!({"text":"你好 hello", "duration":1.25, "n_speakers":1,
        "segments":[{"id":0,"start":0.1,"end":1.2,"speaker":"S1","text":"你好 hello"}],
        "trace_id":"trace", "future_field":true});
    for (format, language) in [
        (Format::Json, None),
        (Format::VerboseJson, Some("zh".into())),
    ] {
        let (url, server) = server(200, expected.to_string().into_bytes(), Duration::ZERO);
        let options = Options { format, language };
        let response = request(
            &client(),
            &url,
            "test-key",
            b"RIFF-audio".to_vec(),
            &options,
        )
        .await
        .unwrap();
        assert_eq!(response, expected);
        let received = String::from_utf8(server.join().unwrap()).unwrap();
        let (headers, body) = received.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("POST /v1/speech_to_text HTTP/1.1"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer test-key")
        );
        assert!(headers.contains("multipart/form-data; boundary="));
        assert_eq!(headers.contains("language: zh"), options.language.is_some());
        for (name, value) in [
            ("model", "asr-1.0"),
            ("response_format", format.as_str()),
            ("stream", "false"),
            ("timestamp_level", "word"),
        ] {
            assert!(
                body.contains(&format!("name=\"{name}\"\r\n\r\n{value}\r\n")),
                "{body}"
            );
        }
        assert!(body.contains("name=\"file\"; filename=\"audio.wav\""));
        assert!(body.contains("Content-Type: audio/wav"));
        assert!(body.contains("RIFF-audio"));
    }
}

#[tokio::test]
async fn preserves_subtitle_bytes_including_unicode_and_newlines() {
    for (format, text) in [
        (
            Format::Srt,
            "1\r\n00:00:00,100 --> 00:00:01,200\r\n你好 hello\r\n\r\n",
        ),
        (
            Format::Vtt,
            "WEBVTT\n\n00:00:00.100 --> 00:00:01.200\nHello\n",
        ),
    ] {
        let (url, server) = server(200, text.as_bytes().to_vec(), Duration::ZERO);
        let value = request(
            &client(),
            &url,
            "test-key",
            vec![],
            &Options {
                format,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(value, Value::String(text.into()));
        server.join().unwrap();
    }
}

#[tokio::test]
async fn reports_http_errors_with_status_and_request_id_without_credentials() {
    for status in [400, 401, 402, 413, 422, 429, 500] {
        let body = json!({"type":"error", "error":{"message":"provider rejected test-key"}, "request_id":"request-123"});
        let (url, server) = server(status, body.to_string().into_bytes(), Duration::ZERO);
        let error = request(&client(), &url, "test-key", vec![], &Options::default())
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with(&format!("{}:", "transcription_api"))
        );
        assert!(error.to_string().contains(&status.to_string()));
        assert!(error.to_string().contains("request-123"));
        assert!(!error.to_string().contains("test-key"));
        assert!(error.to_string().contains("src/transcribe/mod.rs:"));
        server.join().unwrap();
    }
}

#[tokio::test]
async fn rejects_malformed_success_and_handles_non_json_http_failure() {
    for (status, body, code) in [
        (200, b"not json".to_vec(), "invalid_transcription_response"),
        (200, vec![255], "invalid_transcription_response"),
        (200, br#"{"text":"hi","duration":1}"#.to_vec(), "invalid_transcription_response"),
        (200, br#"{"text":"hi","duration":1,"n_speakers":1,"segments":[{"id":0,"start":2,"end":1,"speaker":"S1","text":"hi"}]}"#.to_vec(), "invalid_transcription_response"),
        (500, b"<html>failure</html>".to_vec(), "transcription_api"),
    ] {
        let (url, server) = server(status, body, Duration::ZERO);
        let error = request(&client(), &url, "test-key", vec![], &Options::default()).await.unwrap_err();
        assert!(error.to_string().starts_with(&format!("{}:", code)));
        server.join().unwrap();
    }
}

#[tokio::test]
async fn timeout_is_reported_without_retry() {
    let (url, server) = server(200, b"{}".to_vec(), Duration::from_millis(200));
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let error = request(&client, &url, "test-key", vec![], &Options::default())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}:", "transcription_request"))
    );
    server.join().unwrap();
}

#[tokio::test]
async fn missing_key_is_rejected_before_opening_media() {
    for key in ["", "  "] {
        let error = transcribe(Path::new("does-not-exist.wav"), key, &Options::default())
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with(&format!("{}:", "missing_api_key"))
        );
    }
}

fn client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

fn server(status: u16, body: Vec<u8>, delay: Duration) -> (String, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/v1/speech_to_text",
        listener.local_addr().unwrap()
    );
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            if request.len() >= end + 4 + length {
                break;
            }
        }
        thread::sleep(delay);
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes());
        let _ = socket.write_all(&body);
        request
    });
    (url, handle)
}
