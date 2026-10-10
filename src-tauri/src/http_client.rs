//! The HTTP client used for user-triggered provider requests, such as fetching a
//! vendor's model list (P12).
//!
//! This module only moves bytes. It knows nothing about vendors, models, or the
//! database, and it has no Tauri dependency, so tests can run it against a small
//! local server (see `test_server` at the bottom of this file).
//!
//! Safety rules this module enforces:
//!
//! - **The API key never leaves the `Authorization` header.** It is not put in the
//!   URL, and errors are plain codes (`HttpError`) without any text from the server
//!   or from `reqwest` (whose error messages contain the URL).
//! - **Bounded work.** Every request has a connect timeout and a total timeout, a
//!   caller can add an overall deadline (`with_deadline`), and response bodies are
//!   read chunk by chunk up to a size cap.
//! - **No surprises.** Redirects are never followed, error response bodies are
//!   never read, and production clients use HTTPS only.
//! - **Cancel means stop now.** `run_cancellable` races the request against a
//!   cancel signal; when the signal wins, the request future is dropped and
//!   `reqwest` closes the connection immediately.
//!
//! Production clients use the system proxy: `reqwest` first reads `ALL_PROXY`,
//! `HTTPS_PROXY`, `HTTP_PROXY` and `NO_PROXY` (and their lowercase forms), then the
//! macOS or Windows proxy settings. Linux has no system setting to read, so only the
//! environment variables apply there. Test clients disable every proxy, so a proxy
//! variable on a developer machine or CI runner cannot redirect test traffic.

use std::future::Future;
use std::sync::Once;
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderValue};
use reqwest::{Client, StatusCode, redirect};
use tokio::sync::oneshot;
use url::Url;

use crate::credentials::Secret;

/// How long to wait for a TCP (and TLS) connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest time for one request, from connecting until the body is fully read.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest time for a whole multi-request operation, such as fetching every page
/// of a model list. Callers apply it with `with_deadline`.
pub const OPERATION_DEADLINE: Duration = Duration::from_secs(90);

/// Largest response body accepted, in bytes (8 MiB).
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// How a client chooses a proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    /// Use the proxy environment variables and the macOS/Windows system settings.
    System,
    /// Use no proxy at all, ignoring every environment variable. Tests only.
    Disabled,
}

/// Everything that differs between a production client and a test client.
///
/// Production code can only obtain `HttpSettings::production()`. The test variant
/// exists only in test builds, so a release build cannot accidentally allow plain
/// HTTP or skip the user's proxy.
#[derive(Debug, Clone)]
pub struct HttpSettings {
    proxy: ProxyMode,
    allow_plain_http: bool,
    connect_timeout: Duration,
    request_timeout: Duration,
    max_body_bytes: usize,
}

impl HttpSettings {
    /// Settings for real provider requests: system proxy, HTTPS only, default limits.
    pub fn production() -> Self {
        Self {
            proxy: ProxyMode::System,
            allow_plain_http: false,
            connect_timeout: CONNECT_TIMEOUT,
            request_timeout: REQUEST_TIMEOUT,
            max_body_bytes: MAX_BODY_BYTES,
        }
    }

    /// Settings for the local test server: no proxy, and `http://127.0.0.1` allowed.
    #[cfg(test)]
    pub(crate) fn for_mock_server() -> Self {
        Self {
            proxy: ProxyMode::Disabled,
            allow_plain_http: true,
            ..Self::production()
        }
    }

    /// Shorten the per-request timeout so a timeout test runs quickly.
    #[cfg(test)]
    pub(crate) fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Lower the body cap so a size test does not need megabytes of data.
    #[cfg(test)]
    pub(crate) fn with_max_body_bytes(mut self, max_body_bytes: usize) -> Self {
        self.max_body_bytes = max_body_bytes;
        self
    }

    /// The proxy mode these settings use.
    pub fn proxy(&self) -> ProxyMode {
        self.proxy
    }
}

/// Why a request failed. Each variant is a stable code without any payload, so an
/// error can never carry the URL, a header, the key, or text from the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpError {
    /// The HTTP client could not be built.
    ClientUnavailable,
    /// The URL is not one this client may request (for example plain HTTP).
    InvalidUrl,
    /// The stored key cannot be sent as an HTTP header value.
    SecretInvalid,
    /// DNS, TCP, or proxy connection failure, or the connection broke.
    ConnectionFailed,
    /// The TLS handshake or certificate check failed.
    TlsFailed,
    /// A request or the whole operation took too long.
    RequestTimedOut,
    /// HTTP 401 or 403.
    AuthRejected,
    /// HTTP 402.
    InsufficientBalance,
    /// HTTP 429.
    RateLimited,
    /// HTTP 5xx.
    UpstreamUnavailable,
    /// A redirect or another unexpected status.
    UpstreamResponseInvalid,
    /// The response body is larger than the cap.
    ResponseTooLarge,
    /// The user cancelled the operation.
    Cancelled,
}

impl HttpError {
    /// The stable snake_case code, matching the IPC error codes planned for P12.
    pub fn code(self) -> &'static str {
        match self {
            Self::ClientUnavailable => "operation_failed",
            Self::InvalidUrl => "base_url_invalid",
            Self::SecretInvalid => "secret_invalid",
            Self::ConnectionFailed => "connection_failed",
            Self::TlsFailed => "tls_failed",
            Self::RequestTimedOut => "request_timed_out",
            Self::AuthRejected => "auth_rejected",
            Self::InsufficientBalance => "insufficient_balance",
            Self::RateLimited => "rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::UpstreamResponseInvalid => "upstream_response_invalid",
            Self::ResponseTooLarge => "response_too_large",
            Self::Cancelled => "model_fetch_cancelled",
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only the fixed code is shown; never a URL, header, or server text.
        formatter.write_str(self.code())
    }
}

impl std::error::Error for HttpError {}

/// A configured HTTP client. Build it once and reuse it: it keeps a connection pool.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    settings: HttpSettings,
}

impl HttpClient {
    /// Build a client from the given settings.
    pub fn new(settings: HttpSettings) -> Result<Self, HttpError> {
        install_crypto_provider();
        let builder = Client::builder()
            .connect_timeout(settings.connect_timeout)
            .redirect(redirect::Policy::none())
            .https_only(!settings.allow_plain_http)
            .user_agent(concat!("vibemate/", env!("CARGO_PKG_VERSION")));
        let builder = match settings.proxy {
            // reqwest reads the environment and the system settings by default.
            ProxyMode::System => builder,
            // `no_proxy` clears every proxy and stops reading the environment.
            ProxyMode::Disabled => builder.no_proxy(),
        };
        let client = builder.build().map_err(|_| HttpError::ClientUnavailable)?;
        Ok(Self { client, settings })
    }

    /// Send `GET url` with `Authorization: Bearer <key>` and return the body bytes
    /// of a successful (2xx) response, read up to the size cap.
    ///
    /// Error responses are classified by status code only; their bodies are never
    /// read, because a server might echo the key back.
    pub async fn get_bounded(&self, url: &Url, key: &Secret) -> Result<Vec<u8>, HttpError> {
        self.check_url(url)?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", key.expose()))
            .map_err(|_| HttpError::SecretInvalid)?;
        // Marks the value so the HTTP stack never prints it in debug output.
        authorization.set_sensitive(true);

        let mut response = self
            .client
            .get(url.as_str())
            .header(AUTHORIZATION, authorization)
            .header(ACCEPT, "application/json")
            .timeout(self.settings.request_timeout)
            .send()
            .await
            .map_err(|error| classify(&error))?;

        status_to_result(response.status())?;

        let limit = self.settings.max_body_bytes;
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(HttpError::ResponseTooLarge);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? {
            if body.len() + chunk.len() > limit {
                // Dropping the response here stops reading and closes the connection.
                return Err(HttpError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    /// Reject URLs this client must not request before any network activity.
    fn check_url(&self, url: &Url) -> Result<(), HttpError> {
        let scheme_allowed = match url.scheme() {
            "https" => true,
            "http" => self.settings.allow_plain_http,
            _ => false,
        };
        // Credentials in the URL would leak into logs and proxies; keys use a header.
        let has_credentials = !url.username().is_empty() || url.password().is_some();
        if scheme_allowed && !has_credentials && url.host().is_some() {
            Ok(())
        } else {
            Err(HttpError::InvalidUrl)
        }
    }
}

/// Run `work`, but stop it at once when `cancel` receives a signal.
///
/// When the signal wins, `work` is dropped: an in-flight `reqwest` request closes
/// its connection immediately and nothing after it runs. If the sending side is
/// dropped without sending, nobody can cancel anymore, so `work` simply continues.
///
/// The error type only needs to accept `HttpError::Cancelled`, so callers can use
/// their own error type for work that also parses or validates.
pub async fn run_cancellable<T, E, F>(work: F, mut cancel: oneshot::Receiver<()>) -> Result<T, E>
where
    E: From<HttpError>,
    F: Future<Output = Result<T, E>>,
{
    tokio::pin!(work);
    tokio::select! {
        // `biased` checks the cancel signal first when both sides are ready.
        biased;
        signal = &mut cancel => match signal {
            Ok(()) => Err(HttpError::Cancelled.into()),
            Err(_) => work.await,
        },
        result = &mut work => result,
    }
}

/// Run `work` with an overall deadline; on expiry `work` is dropped and the result
/// is `RequestTimedOut`.
pub async fn with_deadline<T, E, F>(deadline: Duration, work: F) -> Result<T, E>
where
    E: From<HttpError>,
    F: Future<Output = Result<T, E>>,
{
    tokio::time::timeout(deadline, work)
        .await
        .unwrap_or_else(|_| Err(HttpError::RequestTimedOut.into()))
}

/// Install rustls' `ring` crypto provider once per process.
///
/// reqwest's `rustls-no-provider` feature brings rustls without a crypto backend
/// (this keeps aws-lc and its C build out of the app), and building a client
/// without an installed provider panics. `install_default` fails only when a
/// provider is already installed, which is fine, so its result is ignored.
fn install_crypto_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Map an HTTP status to success or a stable error code.
fn status_to_result(status: StatusCode) -> Result<(), HttpError> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(HttpError::AuthRejected),
        402 => Err(HttpError::InsufficientBalance),
        429 => Err(HttpError::RateLimited),
        500..=599 => Err(HttpError::UpstreamUnavailable),
        _ => Err(HttpError::UpstreamResponseInvalid),
    }
}

/// Turn a `reqwest` error into a code. The error itself is dropped unprinted,
/// because its message contains the request URL.
fn classify(error: &reqwest::Error) -> HttpError {
    if error.is_timeout() {
        HttpError::RequestTimedOut
    } else if caused_by_tls(error) {
        HttpError::TlsFailed
    } else if error.is_redirect() {
        HttpError::UpstreamResponseInvalid
    } else if error.is_builder() {
        HttpError::InvalidUrl
    } else {
        HttpError::ConnectionFailed
    }
}

/// Whether a rustls error appears anywhere in the error's cause chain.
///
/// `std::io::Error` hides the error it wraps from `source()`, so it is unwrapped
/// with `get_ref` as well.
fn caused_by_tls(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(cause) = current {
        if cause.downcast_ref::<rustls::Error>().is_some() {
            return true;
        }
        if let Some(inner) = cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            && caused_by_tls(inner)
        {
            return true;
        }
        current = cause.source();
    }
    false
}

/// A tiny HTTP/1.1 server on `127.0.0.1` for tests, built on `std::net::TcpListener`.
///
/// Each accepted connection is answered with the next `Reply` in order. The server
/// records every request head it reads, so tests can check what was sent.
#[cfg(test)]
pub(crate) mod test_server {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use url::Url;

    /// What the server does with one connection.
    pub(crate) enum Reply {
        /// Read the request, then write these bytes and close.
        Respond(Vec<u8>),
        /// Read the request, write `head`, then wait `pause` before closing without
        /// sending the rest of the body.
        Stall { head: Vec<u8>, pause: Duration },
        /// Read the request, then keep the connection open until the client closes it.
        /// The server reports the close on `MockServer::closed`.
        Hang,
        /// Read the request, then close without answering.
        Close,
        /// Write these bytes immediately without reading (for the TLS test).
        Raw(Vec<u8>),
    }

    /// A running test server.
    pub(crate) struct MockServer {
        address: SocketAddr,
        requests: Arc<Mutex<Vec<String>>>,
        /// Receives one message each time a `Hang` connection is closed by the client.
        pub(crate) closed: mpsc::Receiver<()>,
    }

    impl MockServer {
        /// Start a server that answers connections with `replies`, in order.
        pub(crate) fn start(replies: Vec<Reply>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
            let address = listener.local_addr().expect("test server address");
            let requests = Arc::new(Mutex::new(Vec::new()));
            let (closed_sender, closed) = mpsc::channel();
            let recorded = Arc::clone(&requests);
            thread::spawn(move || {
                for reply in replies {
                    let Ok((stream, _)) = listener.accept() else {
                        return;
                    };
                    serve(stream, reply, &recorded, &closed_sender);
                }
            });
            Self {
                address,
                requests,
                closed,
            }
        }

        /// An `http://127.0.0.1:<port><path>` URL for this server.
        pub(crate) fn url(&self, path_and_query: &str) -> Url {
            Url::parse(&format!("http://{}{}", self.address, path_and_query))
                .expect("test server URL")
        }

        /// The same address with the `https` scheme (for the TLS failure test).
        pub(crate) fn https_url(&self, path: &str) -> Url {
            Url::parse(&format!("https://{}{}", self.address, path)).expect("test server URL")
        }

        /// Every request head received so far.
        pub(crate) fn requests(&self) -> Vec<String> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    /// Build a complete HTTP/1.1 response with `Content-Length` and `Connection: close`.
    pub(crate) fn response(status: u16, extra_headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in extra_headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }

    /// A 200 response without `Content-Length`; the body ends when the server closes.
    pub(crate) fn response_until_close(body: &[u8]) -> Vec<u8> {
        let mut bytes = b"HTTP/1.1 200 Test\r\nConnection: close\r\n\r\n".to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    fn serve(
        mut stream: TcpStream,
        reply: Reply,
        requests: &Mutex<Vec<String>>,
        closed: &mpsc::Sender<()>,
    ) {
        // A broken test must fail, not hang forever.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        if let Reply::Raw(bytes) = reply {
            let _ = stream.write_all(&bytes);
            return;
        }
        if let Some(head) = read_head(&mut stream) {
            requests.lock().expect("requests lock").push(head);
        }
        match reply {
            Reply::Respond(bytes) => {
                let _ = stream.write_all(&bytes);
            }
            Reply::Stall { head, pause } => {
                let _ = stream.write_all(&head);
                let _ = stream.flush();
                thread::sleep(pause);
            }
            Reply::Hang => {
                let mut buffer = [0_u8; 64];
                // Returns Ok(0) (or an error) once the client drops the connection.
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                let _ = closed.send(());
            }
            Reply::Close | Reply::Raw(_) => {}
        }
    }

    /// Read bytes until the blank line that ends the request head.
    fn read_head(stream: &mut TcpStream) -> Option<String> {
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match stream.read(&mut byte) {
                Ok(1) => head.push(byte[0]),
                _ => return None,
            }
            if head.len() > 64 * 1024 {
                return None;
            }
        }
        String::from_utf8(head).ok()
    }
}

#[cfg(test)]
mod tests {
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
        let url = Url::parse(&format!("http://127.0.0.1:{port}/models")).unwrap();
        let error = mock_client().get_bounded(&url, &key()).await.unwrap_err();
        assert_eq!(error, HttpError::ConnectionFailed);

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
}
