use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use super::web_assets;
use super::{CliError, errors};

const LOCALHOST: &str = "127.0.0.1:0";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const REQUEST_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) struct WebServerState {
    pub token: String,
    pub result_json: Vec<u8>,
    pub assets_dir: PathBuf,
}

#[allow(dead_code)]
pub(super) struct WebServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    state: WebServerState,
}

#[allow(dead_code)]
impl WebServer {
    pub(super) fn bind(result_json: Vec<u8>, assets_dir: PathBuf) -> Result<Self, CliError> {
        Self::bind_with_state(WebServerState {
            token: generate_token()?,
            result_json,
            assets_dir,
        })
    }

    pub(super) fn graph_url(&self) -> String {
        format!(
            "http://{}/cli/graph?token={}",
            self.local_addr, self.state.token
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
        return HttpResponse::ok("application/json; charset=utf-8", state.result_json.clone());
    }

    if path == "/cli/graph" {
        if !token_matches(target, &state.token) {
            return HttpResponse::forbidden();
        }
        return asset_response(state, target);
    }

    asset_response(state, target)
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

fn asset_response(state: &WebServerState, target: &str) -> HttpResponse {
    web_assets::find_asset(&state.assets_dir, target).map_or_else(
        HttpResponse::not_found,
        |asset| match web_assets::read_asset_bytes(&asset) {
            Ok(bytes) => HttpResponse::ok(asset.content_type, bytes),
            Err(error) => HttpResponse::text(
                "500 Internal Server Error",
                &format!("failed to read asset: {error}\n"),
            ),
        },
    )
}

fn generate_token() -> Result<String, CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| CliError::WebServer(format!("failed to generate web token: {error}")))?;
    Ok(hex_encode(&bytes))
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
    body: Vec<u8>,
}

impl HttpResponse {
    fn ok(content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            content_type,
            body,
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
            body: body.as_bytes().to_vec(),
        }
    }

    fn write_to(&self, stream: &mut TcpStream) -> Result<(), CliError> {
        write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            self.content_type,
            self.body.len()
        )
        .map_err(|error| CliError::WebServer(error.to_string()))?;
        stream
            .write_all(&self.body)
            .map_err(|error| CliError::WebServer(error.to_string()))
    }
}
