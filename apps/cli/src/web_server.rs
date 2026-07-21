use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use super::web_payload::CliGraphMetadataPayload;
use super::{CliError, errors};

include!(concat!(env!("OUT_DIR"), "/treease_web_config.rs"));

const LOCALHOST: &str = "127.0.0.1:0";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const REQUEST_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) struct WebServerState {
    pub token: String,
    pub result: WebServerResult,
}

#[derive(Debug, Clone)]
pub(super) struct WebServerResult {
    pub source_label: String,
    pub expression: String,
    pub language: String,
    pub source: WebServerSource,
}

#[derive(Debug, Clone)]
pub(super) enum WebServerSource {
    Text(String),
    File(PathBuf),
}

#[allow(dead_code)]
pub(super) struct WebServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    state: WebServerState,
}

#[allow(dead_code)]
impl WebServer {
    pub(super) fn bind(result: WebServerResult) -> Result<Self, CliError> {
        Self::bind_with_state(WebServerState {
            token: generate_token()?,
            result,
        })
    }

    pub(super) fn editor_url(&self) -> String {
        let api_url = format!("http://{}", self.local_addr);
        let source_url = format!("{}/cli/source?token={}", api_url, self.state.token);
        format!(
            "{}?textUrl={}&lang={}&ui=editor%2Cviewer",
            DEFAULT_WEB_URL,
            percent_encode(&source_url),
            percent_encode(&self.state.result.language),
        )
    }

    pub(super) fn serve_forever(self) -> Result<(), CliError> {
        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    if let Err(error) = serve_connection(&self.state, &mut stream) {
                        eprintln!(
                            "treease web request failed: {}",
                            errors::render_text(&error)
                        );
                    }
                }
                Err(error) => {
                    eprintln!("treease web accept failed: {error}");
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn bind_for_test(state: WebServerState) -> Result<Self, CliError> {
        Self::bind_with_state(state)
    }

    #[cfg(test)]
    pub(super) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    #[cfg(test)]
    pub(super) fn serve_once_for_test(self) -> Result<(), CliError> {
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(|error| CliError::WebServer(error.to_string()))?;
        serve_connection(&self.state, &mut stream)
    }

    fn bind_with_state(state: WebServerState) -> Result<Self, CliError> {
        let listener =
            TcpListener::bind(LOCALHOST).map_err(|error| CliError::WebServer(error.to_string()))?;
        let local_addr = listener
            .local_addr()
            .map_err(|error| CliError::WebServer(error.to_string()))?;
        Ok(Self {
            listener,
            local_addr,
            state,
        })
    }
}

fn serve_connection(state: &WebServerState, stream: &mut TcpStream) -> Result<(), CliError> {
    stream
        .set_read_timeout(Some(REQUEST_IO_TIMEOUT))
        .map_err(|error| CliError::WebServer(error.to_string()))?;
    stream
        .set_write_timeout(Some(REQUEST_IO_TIMEOUT))
        .map_err(|error| CliError::WebServer(error.to_string()))?;
    let request = read_http_request(stream)?;
    let response = handle_request(state, &request);
    response.write_to(stream)
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, CliError> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];

    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| CliError::WebServer(error.to_string()))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if has_header_terminator(&request) {
            break;
        }
        if request.len() > MAX_REQUEST_BYTES {
            return Ok("GET /__treease_request_too_large__ HTTP/1.1\r\n\r\n".to_string());
        }
    }

    Ok(String::from_utf8_lossy(&request).into_owned())
}

fn has_header_terminator(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| window == b"\r\n\r\n")
        || bytes.windows(2).any(|window| window == b"\n\n")
}

fn handle_request(state: &WebServerState, request: &str) -> HttpResponse {
    let Some((method, target)) = parse_request_line(request) else {
        return HttpResponse::bad_request();
    };
    if method != "GET" {
        return HttpResponse::method_not_allowed();
    }

    let path = request_path(target);
    if path == "/cli/result" {
        if !token_matches(target, &state.token) {
            return HttpResponse::forbidden();
        }
        let result_json = match state.result.legacy_result_json() {
            Ok(bytes) => bytes,
            Err(error) => {
                return HttpResponse::text(
                    "500 Internal Server Error",
                    &format!(
                        "failed to serialize CLI graph result: {}\n",
                        errors::render_text(&error)
                    ),
                );
            }
        };
        return HttpResponse::ok("application/json; charset=utf-8", result_json);
    }

    if path == "/cli/meta" {
        if !token_matches(target, &state.token) {
            return HttpResponse::forbidden();
        }
        let metadata = state.result.metadata(format!(
            "/cli/source?token={}",
            percent_encode(&state.token)
        ));
        let metadata_json = match serde_json::to_vec(&metadata) {
            Ok(bytes) => bytes,
            Err(error) => {
                return HttpResponse::text(
                    "500 Internal Server Error",
                    &format!("failed to serialize CLI graph metadata: {error}\n"),
                );
            }
        };
        return HttpResponse::ok("application/json; charset=utf-8", metadata_json);
    }

    if path == "/cli/source" {
        if !token_matches(target, &state.token) {
            return HttpResponse::forbidden();
        }
        return HttpResponse::source(
            source_content_type(&state.result.language),
            state.result.source.clone(),
        );
    }

    HttpResponse::not_found()
}

fn source_content_type(language: &str) -> &'static str {
    match language {
        "json" => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

impl WebServerResult {
    pub(super) fn text(
        source_label: String,
        expression: String,
        language: String,
        text: String,
    ) -> Self {
        Self {
            source_label,
            expression,
            language,
            source: WebServerSource::Text(text),
        }
    }

    pub(super) fn file(
        source_label: String,
        expression: String,
        language: String,
        path: PathBuf,
    ) -> Self {
        Self {
            source_label,
            expression,
            language,
            source: WebServerSource::File(path),
        }
    }

    fn metadata(&self, source_url: String) -> CliGraphMetadataPayload {
        CliGraphMetadataPayload {
            source_label: self.source_label.clone(),
            expression: self.expression.clone(),
            language: self.language.clone(),
            source_url,
            byte_length: self.source.byte_length().unwrap_or(0),
        }
    }

    fn legacy_result_json(&self) -> Result<Vec<u8>, CliError> {
        let payload = serde_json::json!({
            "source_label": self.source_label,
            "expression": self.expression,
            "language": self.language,
            "text": self.source.read_to_string()?,
        });
        serde_json::to_vec(&payload).map_err(|error| CliError::Eval(error.to_string()))
    }
}

impl WebServerSource {
    fn byte_length(&self) -> Result<usize, CliError> {
        match self {
            WebServerSource::Text(text) => Ok(text.len()),
            WebServerSource::File(path) => {
                Ok(std::fs::metadata(path).map_err(CliError::Io)?.len() as usize)
            }
        }
    }

    fn read_to_string(&self) -> Result<String, CliError> {
        match self {
            WebServerSource::Text(text) => Ok(text.clone()),
            WebServerSource::File(path) => std::fs::read_to_string(path).map_err(CliError::Io),
        }
    }
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    Some((method, target))
}

fn request_path(target: &str) -> &str {
    target.split_once('?').map_or(target, |(path, _)| path)
}

fn token_matches(target: &str, expected: &str) -> bool {
    let Some((_, query)) = target.split_once('?') else {
        return false;
    };
    query.split('&').any(|pair| {
        let Some((key, value)) = pair.split_once('=') else {
            return false;
        };
        key == "token" && value == expected
    })
}

fn generate_token() -> Result<String, CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| CliError::WebServer(format!("failed to generate web token: {error}")))?;
    Ok(hex_encode(&bytes))
}

fn percent_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

struct HttpResponse {
    status: &'static str,
    content_type: &'static str,
    body: HttpResponseBody,
}

enum HttpResponseBody {
    Bytes(Vec<u8>),
    Source(WebServerSource),
}

impl HttpResponse {
    fn ok(content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            content_type,
            body: HttpResponseBody::Bytes(body),
        }
    }

    fn source(content_type: &'static str, source: WebServerSource) -> Self {
        Self {
            status: "200 OK",
            content_type,
            body: HttpResponseBody::Source(source),
        }
    }

    fn bad_request() -> Self {
        Self::text("400 Bad Request", "bad request\n")
    }

    fn forbidden() -> Self {
        Self::text("403 Forbidden", "forbidden\n")
    }

    fn not_found() -> Self {
        Self::text("404 Not Found", "not found\n")
    }

    fn method_not_allowed() -> Self {
        Self::text("405 Method Not Allowed", "method not allowed\n")
    }

    fn text(status: &'static str, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: HttpResponseBody::Bytes(body.as_bytes().to_vec()),
        }
    }

    fn write_to(&self, stream: &mut TcpStream) -> Result<(), CliError> {
        let content_length = match &self.body {
            HttpResponseBody::Bytes(bytes) => bytes.len(),
            HttpResponseBody::Source(source) => source.byte_length()?,
        };
        write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status, self.content_type, content_length
        )
        .map_err(|error| CliError::WebServer(error.to_string()))?;
        match &self.body {
            HttpResponseBody::Bytes(bytes) => stream
                .write_all(bytes)
                .map_err(|error| CliError::WebServer(error.to_string())),
            HttpResponseBody::Source(WebServerSource::Text(text)) => stream
                .write_all(text.as_bytes())
                .map_err(|error| CliError::WebServer(error.to_string())),
            HttpResponseBody::Source(WebServerSource::File(path)) => {
                let mut file = File::open(path).map_err(CliError::Io)?;
                std::io::copy(&mut file, stream)
                    .map(|_| ())
                    .map_err(|error| CliError::WebServer(error.to_string()))
            }
        }
    }
}
