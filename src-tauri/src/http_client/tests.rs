//! Behavior tests for the parent module, kept separate from runtime code.

use std::io;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use super::test_server::{MockServer, Reply, response, response_until_close};
use super::*;

const KEY: &str = "sk-test-SECRET-value-123";

fn key() -> Secret {
    Secret::new(KEY.to_owned())
}

/// Wait up to five seconds for the server to see the client close a `Hang`
/// connection. It polls instead of blocking, so the runtime can keep running the
/// client's connection task, which is what closes the socket.
async fn closed_by_client(server: &MockServer) -> bool {
    for _ in 0..500 {
        if server.closed.try_recv().is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

fn mock_client() -> HttpClient {
    HttpClient::new(HttpSettings::for_mock_server()).expect("test client")
}

/// Assert that no text derived from an error contains the key.
fn assert_redacted(error: HttpError) {
    for text in [
        error.to_string(),
        format!("{error:?}"),
        error.code().to_owned(),
    ] {
        assert!(!text.contains(KEY), "key leaked in {text}");
        assert!(!text.contains("SECRET"), "key fragment leaked in {text}");
    }
}

#[tokio::test]
async fn returns_the_body_and_sends_the_key_only_in_the_authorization_header() {
    let server = MockServer::start(vec![Reply::Respond(response(
        200,
        &[("Content-Type", "application/json")],
        br#"{"data":[]}"#,
    ))]);
    let body = mock_client()
        .get_bounded(&server.url("/v1/models?limit=500"), &key())
        .await
        .expect("success");
    assert_eq!(body, br#"{"data":[]}"#);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let head = &requests[0];
    let request_line = head.lines().next().expect("request line");
    assert_eq!(request_line, "GET /v1/models?limit=500 HTTP/1.1");
    let lower = head.to_ascii_lowercase();
    assert!(lower.contains(&format!(
        "authorization: bearer {}",
        KEY.to_ascii_lowercase()
    )));
    assert!(lower.contains("accept: application/json"));
    assert!(lower.contains("user-agent: vibemate/"));
    assert_eq!(head.matches(KEY).count(), 1, "key must appear exactly once");
}

#[tokio::test]
async fn maps_error_statuses_without_reading_echoed_secrets() {
    let echo = format!("{{\"error\":\"bad key {KEY}\"}}");
    let cases = [
        (401, HttpError::AuthRejected),
        (403, HttpError::AuthRejected),
        (402, HttpError::InsufficientBalance),
        (429, HttpError::RateLimited),
        (500, HttpError::UpstreamUnavailable),
        (503, HttpError::UpstreamUnavailable),
        (404, HttpError::UpstreamResponseInvalid),
        (400, HttpError::UpstreamResponseInvalid),
    ];
    for (status, expected) in cases {
        let server = MockServer::start(vec![Reply::Respond(response(
            status,
            &[("X-Echo", KEY)],
            echo.as_bytes(),
        ))]);
        let error = mock_client()
            .get_bounded(&server.url("/models"), &key())
            .await
            .expect_err("error status");
        assert_eq!(error, expected, "status {status}");
        assert_redacted(error);
    }
}

#[tokio::test]
async fn does_not_follow_redirects() {
    let location = format!("http://127.0.0.1:9/steal?key={KEY}");
    let server = MockServer::start(vec![
        Reply::Respond(response(302, &[("Location", &location)], b"")),
        Reply::Respond(response(200, &[], b"{}")),
    ]);
    let error = mock_client()
        .get_bounded(&server.url("/models"), &key())
        .await
        .expect_err("redirect");
    assert_eq!(error, HttpError::UpstreamResponseInvalid);
    assert_redacted(error);
    assert_eq!(
        server.requests().len(),
        1,
        "the redirect must not be followed"
    );
}

#[tokio::test]
async fn rejects_a_body_whose_declared_length_is_over_the_cap() {
    let server = MockServer::start(vec![Reply::Respond(response(200, &[], &[b'x'; 2048]))]);
    let client =
        HttpClient::new(HttpSettings::for_mock_server().with_max_body_bytes(1024)).unwrap();
    let error = client
        .get_bounded(&server.url("/models"), &key())
        .await
        .expect_err("too large");
    assert_eq!(error, HttpError::ResponseTooLarge);
}

#[tokio::test]
async fn stops_reading_an_undeclared_body_at_the_cap() {
    let server = MockServer::start(vec![Reply::Respond(response_until_close(&[b'x'; 4096]))]);
    let client =
        HttpClient::new(HttpSettings::for_mock_server().with_max_body_bytes(1024)).unwrap();
    let error = client
        .get_bounded(&server.url("/models"), &key())
        .await
        .expect_err("too large");
    assert_eq!(error, HttpError::ResponseTooLarge);
}

#[tokio::test]
async fn accepts_a_body_exactly_at_the_cap() {
    let server = MockServer::start(vec![Reply::Respond(response_until_close(&[b'x'; 1024]))]);
    let client =
        HttpClient::new(HttpSettings::for_mock_server().with_max_body_bytes(1024)).unwrap();
    let body = client
        .get_bounded(&server.url("/models"), &key())
        .await
        .expect("at the cap");
    assert_eq!(body.len(), 1024);
}

#[tokio::test]
async fn times_out_a_stalled_body() {
    let head = b"HTTP/1.1 200 Test\r\nContent-Length: 100\r\n\r\n{\"data\"".to_vec();
    let server = MockServer::start(vec![Reply::Stall {
        head,
        pause: Duration::from_secs(3),
    }]);
    let client = HttpClient::new(
        HttpSettings::for_mock_server().with_request_timeout(Duration::from_millis(300)),
    )
    .unwrap();
    let started = Instant::now();
    let error = client
        .get_bounded(&server.url("/models"), &key())
        .await
        .expect_err("timeout");
    assert_eq!(error, HttpError::RequestTimedOut);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn reports_a_refused_or_dropped_connection() {
    // Bind and release a port so nothing listens on it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    // An HTTPS URL alone does not make a TCP refusal a TLS failure.
    for scheme in ["http", "https"] {
        let url = Url::parse(&format!("{scheme}://127.0.0.1:{port}/models")).unwrap();
        let error = mock_client().get_bounded(&url, &key()).await.unwrap_err();
        assert_eq!(error, HttpError::ConnectionFailed);
    }

    let server = MockServer::start(vec![Reply::Close]);
    let error = mock_client()
        .get_bounded(&server.url("/models"), &key())
        .await
        .unwrap_err();
    assert_eq!(error, HttpError::ConnectionFailed);
}

#[tokio::test]
async fn reports_a_failed_tls_handshake() {
    // The server answers a TLS ClientHello with plain HTTP, which rustls rejects.
    let server = MockServer::start(vec![Reply::Raw(response(200, &[], b"{}"))]);
    let error = mock_client()
        .get_bounded(&server.https_url("/models"), &key())
        .await
        .expect_err("TLS failure");
    assert_eq!(error, HttpError::TlsFailed);
    assert_redacted(error);
}

#[test]
fn recognizes_tls_causes_wrapped_in_io_errors() {
    let tls_error = rustls::Error::AlertReceived(rustls::AlertDescription::HandshakeFailure);
    assert!(caused_by_tls(&tls_error));

    // io::Error::source skips its own payload, so checking source() alone
    // would miss this rustls error, including behind another I/O wrapper.
    let io_error = io::Error::new(io::ErrorKind::InvalidData, tls_error);
    assert!(caused_by_tls(&io_error));
    assert!(caused_by_tls(&io::Error::other(io_error)));
}

#[test]
fn does_not_treat_transport_errors_as_tls_failures() {
    for kind in [
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::ConnectionRefused,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::TimedOut,
    ] {
        let error = io::Error::from(kind);
        assert!(!caused_by_tls(&error), "transport error: {kind:?}");
        assert!(
            !caused_by_tls(&io::Error::other(error)),
            "wrapped transport error: {kind:?}"
        );
    }
}

#[tokio::test]
async fn production_settings_refuse_plain_http_and_url_credentials() {
    let client = HttpClient::new(HttpSettings::production()).unwrap();
    assert_eq!(HttpSettings::production().proxy(), ProxyMode::System);
    for url in [
        "http://127.0.0.1:9/models",
        "https://user:pass@example.com/models",
        "ftp://example.com/models",
    ] {
        let error = client
            .get_bounded(&Url::parse(url).unwrap(), &key())
            .await
            .unwrap_err();
        assert_eq!(error, HttpError::InvalidUrl, "{url}");
    }
}

#[tokio::test]
async fn rejects_a_key_that_cannot_be_a_header_value() {
    let server = MockServer::start(vec![]);
    let error = mock_client()
        .get_bounded(&server.url("/models"), &Secret::new("bad\nkey".to_owned()))
        .await
        .unwrap_err();
    assert_eq!(error, HttpError::SecretInvalid);
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn cancelling_aborts_the_in_flight_request_immediately() {
    let server = MockServer::start(vec![Reply::Hang]);
    let client = mock_client();
    let url = server.url("/models");
    let secret = key();
    let (cancel, cancelled) = oneshot::channel();

    let canceller = async {
        // Wait until the server has the request, then cancel.
        while server.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        cancel.send(()).unwrap();
    };
    let started = Instant::now();
    let (result, ()) = tokio::join!(
        run_cancellable(client.get_bounded(&url, &secret), cancelled),
        canceller
    );
    assert_eq!(result, Err(HttpError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));

    // The server sees the connection close: the request was really aborted.
    assert!(
        closed_by_client(&server).await,
        "connection closed after cancel"
    );
}

#[tokio::test]
async fn a_dropped_cancel_sender_does_not_cancel() {
    let server = MockServer::start(vec![Reply::Respond(response(200, &[], b"{}"))]);
    let client = mock_client();
    let (cancel, cancelled) = oneshot::channel::<()>();
    drop(cancel);
    let body = run_cancellable(
        client.get_bounded(&server.url("/models"), &key()),
        cancelled,
    )
    .await
    .expect("work continues");
    assert_eq!(body, b"{}");
}

#[tokio::test]
async fn the_overall_deadline_stops_a_hanging_request() {
    let server = MockServer::start(vec![Reply::Hang]);
    let client = mock_client();
    let started = Instant::now();
    let result = with_deadline(
        Duration::from_millis(200),
        client.get_bounded(&server.url("/models"), &key()),
    )
    .await;
    assert_eq!(result, Err(HttpError::RequestTimedOut));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        closed_by_client(&server).await,
        "connection closed after the deadline"
    );
}
