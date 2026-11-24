//! Bounded HTTP transport for the AI host layer.
//!
//! Loopback HTTP uses `std::net`; HTTPS uses the system `curl` binary with
//! explicit arguments and no shell. The transport enforces the network policy,
//! header hygiene, timeouts, and response-size limits. It does not interpret
//! provider payloads.

use super::provider::{
    AI_E_CONFIG, AI_E_CONNECT, AI_E_IO, AI_E_NETWORK_DISABLED, AI_E_PROVIDER,
    AI_E_RESPONSE_TOO_LARGE, AI_E_TIMEOUT, AiError,
};
use super::redact;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_HEADERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlScheme {
    Http,
    Https,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedUrl {
    pub scheme: UrlScheme,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl ParsedUrl {
    pub fn is_loopback(&self) -> bool {
        if self.host.eq_ignore_ascii_case("localhost") {
            return true;
        }
        self.host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    }

    pub fn host_header(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let default_port = match self.scheme {
            UrlScheme::Http => 80,
            UrlScheme::Https => 443,
        };
        if self.port == default_port {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }
}

/// Parses an absolute HTTP(S) URL. Credentials in the URL, fragments, and
/// control characters are rejected.
pub fn parse_url(url: &str) -> Result<ParsedUrl, AiError> {
    if url.len() > 2048 {
        return Err(AiError::new(AI_E_CONFIG, "URL is longer than 2048 bytes"));
    }
    if url
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AiError::new(
            AI_E_CONFIG,
            "URL must not contain whitespace or control characters",
        ));
    }
    let (scheme, rest) = url.split_once("://").ok_or_else(|| {
        AiError::new(
            AI_E_CONFIG,
            "URL must be absolute and start with http:// or https://",
        )
    })?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "http" => UrlScheme::Http,
        "https" => UrlScheme::Https,
        _ => {
            return Err(AiError::new(
                AI_E_CONFIG,
                "only http and https URLs are supported",
            ));
        }
    };
    if rest.is_empty() {
        return Err(AiError::new(AI_E_CONFIG, "URL is missing a host"));
    }
    if rest.contains('#') {
        return Err(AiError::new(AI_E_CONFIG, "URL fragments are not supported"));
    }
    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let raw_path = &rest[authority_end..];
    if authority.is_empty() {
        return Err(AiError::new(AI_E_CONFIG, "URL is missing a host"));
    }
    if authority.contains('@') {
        return Err(AiError::new(
            AI_E_CONFIG,
            "credentials in URLs are not supported; use an explicit API-key variable",
        ));
    }

    let (host, port_text) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| AiError::new(AI_E_CONFIG, "unterminated IPv6 address in URL"))?;
        let port = after.strip_prefix(':');
        if !after.is_empty() && port.is_none() {
            return Err(AiError::new(AI_E_CONFIG, "invalid IPv6 URL authority"));
        }
        (host, port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return Err(AiError::new(
                AI_E_CONFIG,
                "IPv6 addresses must be enclosed in brackets",
            ));
        }
        (host, Some(port))
    } else {
        (authority, None)
    };
    if host.is_empty() {
        return Err(AiError::new(AI_E_CONFIG, "URL is missing a host"));
    }
    if host.parse::<IpAddr>().is_err() && !host.eq_ignore_ascii_case("localhost") {
        let valid = host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'));
        if !valid || host.starts_with('.') || host.ends_with('.') {
            return Err(AiError::new(AI_E_CONFIG, "invalid host name in URL"));
        }
    }
    let default_port = match scheme {
        UrlScheme::Http => 80,
        UrlScheme::Https => 443,
    };
    let port = match port_text {
        Some(text) if !text.is_empty() => text
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| AiError::new(AI_E_CONFIG, "invalid port in URL"))?,
        _ => default_port,
    };
    let path = if raw_path.is_empty() {
        "/".to_string()
    } else if raw_path.starts_with('/') {
        raw_path.to_string()
    } else {
        format!("/{raw_path}")
    };
    Ok(ParsedUrl {
        scheme,
        host: host.to_string(),
        port,
        path,
    })
}

/// Host-level network policy. Localhost HTTP is allowed without a flag;
/// non-loopback endpoints require HTTPS and an explicit opt-in.
#[derive(Clone, Debug, Default)]
pub struct NetworkPolicy {
    allow_network: bool,
    allowed_hosts: Vec<String>,
}

impl NetworkPolicy {
    pub fn local_only() -> Self {
        Self::default()
    }

    pub fn with_network() -> Self {
        Self {
            allow_network: true,
            allowed_hosts: Vec::new(),
        }
    }

    pub fn with_allowed_host(mut self, host: impl Into<String>) -> Self {
        self.allowed_hosts.push(host.into());
        self
    }

    pub fn allows_network(&self) -> bool {
        self.allow_network
    }

    pub fn check(&self, url: &ParsedUrl) -> Result<(), AiError> {
        if !self.allowed_hosts.is_empty() {
            let host = url.host.to_ascii_lowercase();
            let host_port = format!("{host}:{}", url.port);
            let allowed = self.allowed_hosts.iter().any(|allowed| {
                let allowed = allowed.to_ascii_lowercase();
                allowed == host || allowed == host_port
            });
            if !allowed {
                return Err(AiError::new(
                    AI_E_NETWORK_DISABLED,
                    format!(
                        "host `{}` is not in the allowlist; add it explicitly before retrying",
                        url.host
                    ),
                ));
            }
        }
        match url.scheme {
            UrlScheme::Http if !url.is_loopback() => Err(AiError::new(
                AI_E_NETWORK_DISABLED,
                format!(
                    "refusing plaintext HTTP to non-loopback host `{}`; use https://",
                    url.host
                ),
            )),
            UrlScheme::Https if !self.allow_network => Err(AiError::new(
                AI_E_NETWORK_DISABLED,
                "remote HTTPS is disabled; pass --allow-network (or set VANTYX_AI_ALLOW_NETWORK=1)",
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    }
}

pub trait Transport {
    fn execute(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<HttpResponse, AiError>;

    /// Executes a request with an optional per-request timeout. The transport
    /// may cap the requested value; it must never extend its global limit.
    fn execute_with_timeout(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<HttpResponse, AiError> {
        let _ = timeout;
        self.execute(method, url, headers, body)
    }
}

pub struct HttpTransport {
    policy: NetworkPolicy,
    timeout: Duration,
    max_response_bytes: usize,
}

impl HttpTransport {
    pub fn new(policy: NetworkPolicy) -> Self {
        Self {
            policy,
            timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout.max(Duration::from_millis(1));
        self
    }

    pub fn with_max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = max_response_bytes.max(1);
        self
    }

    pub fn policy(&self) -> &NetworkPolicy {
        &self.policy
    }

    pub fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<HttpResponse, AiError> {
        self.request_with_timeout(method, url, headers, body, None)
    }

    pub fn request_with_timeout(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
        requested_timeout: Option<Duration>,
    ) -> Result<HttpResponse, AiError> {
        if !matches!(method, "GET" | "POST") {
            return Err(AiError::new(
                AI_E_CONFIG,
                "only GET and POST requests are supported",
            ));
        }
        if headers.len() > MAX_HEADERS {
            return Err(AiError::new(AI_E_CONFIG, "too many request headers"));
        }
        for (name, value) in headers {
            validate_header(name, value)?;
        }
        if body.is_some_and(|body| body.len() > MAX_REQUEST_BYTES) {
            return Err(AiError::new(
                AI_E_CONFIG,
                format!("request body exceeds the {MAX_REQUEST_BYTES} byte limit"),
            ));
        }
        if method == "GET" && body.is_some_and(|body| !body.is_empty()) {
            return Err(AiError::new(
                AI_E_CONFIG,
                "GET requests must not have a body",
            ));
        }
        let parsed = parse_url(url)?;
        self.policy.check(&parsed)?;
        let timeout = requested_timeout
            .map(|timeout| timeout.clamp(Duration::from_millis(1), Duration::from_secs(3600)))
            .unwrap_or(self.timeout);
        match parsed.scheme {
            UrlScheme::Http => self.send_loopback(&parsed, method, headers, body, timeout),
            UrlScheme::Https => self.send_curl(&parsed, url, method, headers, body, timeout),
        }
    }

    fn send_loopback(
        &self,
        parsed: &ParsedUrl,
        method: &str,
        headers: &[(String, String)],
        body: Option<&str>,
        timeout: Duration,
    ) -> Result<HttpResponse, AiError> {
        let deadline = Instant::now() + timeout;
        let addresses = (parsed.host.as_str(), parsed.port)
            .to_socket_addrs()
            .map_err(|error| {
                AiError::new(
                    AI_E_CONNECT,
                    format!("cannot resolve `{}`: {error}", parsed.host),
                )
            })?
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(AiError::new(
                AI_E_CONNECT,
                format!("no address found for `{}`", parsed.host),
            ));
        }
        let mut last_error = None;
        let mut stream = None;
        for address in addresses {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(10))) {
                Ok(connected) => {
                    stream = Some(connected);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let mut stream = stream.ok_or_else(|| {
            let detail = last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "connection deadline exceeded".to_string());
            AiError::new(
                AI_E_CONNECT,
                format!("cannot connect to {}: {detail}", parsed.host_header()),
            )
        })?;
        let _ = stream.set_write_timeout(Some(timeout));

        let mut request = format!(
            "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: vantyx-ai/0.1\r\nAccept: application/json\r\n",
            parsed.path,
            parsed.host_header()
        );
        let mut has_content_type = false;
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("content-type") {
                has_content_type = true;
            }
            request.push_str(name);
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }
        if let Some(body) = body {
            if !has_content_type {
                request.push_str("Content-Type: application/json\r\n");
            }
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        request.push_str("\r\n");
        if let Some(body) = body {
            request.push_str(body);
        }
        stream.write_all(request.as_bytes()).map_err(|error| {
            AiError::new(AI_E_CONNECT, format!("failed to send request: {error}"))
        })?;
        stream.flush().map_err(|error| {
            AiError::new(AI_E_CONNECT, format!("failed to flush request: {error}"))
        })?;
        read_response(&mut stream, deadline, self.max_response_bytes)
    }

    fn send_curl(
        &self,
        parsed: &ParsedUrl,
        url: &str,
        method: &str,
        headers: &[(String, String)],
        body: Option<&str>,
        timeout: Duration,
    ) -> Result<HttpResponse, AiError> {
        let (stderr_path, stderr_file) = unique_temp_file()?;
        let seconds = timeout.as_secs().clamp(1, 3600).to_string();
        let connect_seconds = timeout.as_secs().clamp(1, 10).to_string();
        let mut command = Command::new("curl");
        command
            .arg("--silent")
            .arg("--show-error")
            .arg("--proto")
            .arg("=https")
            .arg("--proto-redir")
            .arg("=https")
            .arg("--max-redirs")
            .arg("0")
            .arg("--max-time")
            .arg(&seconds)
            .arg("--connect-timeout")
            .arg(&connect_seconds)
            .arg("--request")
            .arg(method)
            .arg("--header")
            .arg("Accept: application/json")
            .arg("--write-out")
            .arg("\n%{http_code}");
        for (name, value) in headers {
            command.arg("--header").arg(format!("{name}: {value}"));
        }
        if body.is_some() {
            command.arg("--data-binary").arg("@-");
        }
        command
            .arg(url)
            .stdin(if body.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr_file));

        let mut child = command.spawn().map_err(|error| {
            let _ = fs::remove_file(&stderr_path);
            if error.kind() == std::io::ErrorKind::NotFound {
                AiError::new(
                    AI_E_CONNECT,
                    "HTTPS requires the system `curl` binary; install curl or use a loopback HTTP endpoint",
                )
            } else {
                AiError::new(AI_E_CONNECT, format!("cannot start curl: {error}"))
            }
        })?;
        if let Some(body) = body {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| AiError::new(AI_E_CONNECT, "cannot open curl stdin"))?;
            stdin.write_all(body.as_bytes()).map_err(|error| {
                AiError::new(AI_E_CONNECT, format!("cannot write request body: {error}"))
            })?;
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AiError::new(AI_E_CONNECT, "cannot read curl stdout"))?;
        let mut output = Vec::new();
        let read_limit = self.max_response_bytes.saturating_add(129) as u64;
        let read_result = stdout.take(read_limit).read_to_end(&mut output);
        if read_result.is_err() || output.len() > self.max_response_bytes.saturating_add(128) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&stderr_path);
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!(
                    "response exceeds the {} byte limit",
                    self.max_response_bytes
                ),
            ));
        }
        let status = child.wait().map_err(|error| {
            AiError::new(AI_E_CONNECT, format!("cannot wait for curl: {error}"))
        })?;
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        let _ = fs::remove_file(&stderr_path);
        if !status.success() {
            return Err(AiError::new(
                AI_E_CONNECT,
                format!(
                    "curl failed for {}: {}",
                    parsed.host_header(),
                    redact::sanitize(&stderr, &[], 512)
                ),
            ));
        }
        let text = String::from_utf8(output)
            .map_err(|_| AiError::new(AI_E_PROVIDER, "HTTPS response is not valid UTF-8"))?;
        let split = text.rfind('\n').ok_or_else(|| {
            AiError::new(
                AI_E_PROVIDER,
                "curl response did not include an HTTP status",
            )
        })?;
        let (body, status_text) = text.split_at(split);
        let status = status_text
            .trim()
            .parse::<u16>()
            .map_err(|_| AiError::new(AI_E_PROVIDER, "curl returned an invalid HTTP status"))?;
        if body.len() > self.max_response_bytes {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!(
                    "response exceeds the {} byte limit",
                    self.max_response_bytes
                ),
            ));
        }
        Ok(HttpResponse {
            status,
            headers: Vec::new(),
            body: body.to_string(),
        })
    }
}

impl Transport for HttpTransport {
    fn execute(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<HttpResponse, AiError> {
        self.request(method, url, headers, body)
    }

    fn execute_with_timeout(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<HttpResponse, AiError> {
        self.request_with_timeout(method, url, headers, body, timeout)
    }
}

fn validate_header(name: &str, value: &str) -> Result<(), AiError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(AiError::new(AI_E_CONFIG, "invalid HTTP header name"));
    }
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "content-length" | "connection" | "transfer-encoding"
    ) {
        return Err(AiError::new(
            AI_E_CONFIG,
            format!("header `{name}` is managed by the transport"),
        ));
    }
    if value.contains('\r') || value.contains('\n') || value.len() > 4096 {
        return Err(AiError::new(
            AI_E_CONFIG,
            format!("invalid value for header `{name}`"),
        ));
    }
    Ok(())
}

fn read_response(
    stream: &mut TcpStream,
    deadline: Instant,
    max_response_bytes: usize,
) -> Result<HttpResponse, AiError> {
    let (rest, status, headers) = read_headers(stream, deadline)?;
    let chunked = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding")
            && value.to_ascii_lowercase().contains("chunked")
    });
    let content_length = headers
        .iter()
        .find_map(|(name, value)| name.eq_ignore_ascii_case("content-length").then_some(value))
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| AiError::new(AI_E_PROVIDER, "invalid Content-Length in response"))
        })
        .transpose()?;

    let body = if chunked {
        let mut encoded = rest;
        read_to_end_bounded(
            stream,
            &mut encoded,
            max_response_bytes + 64 * 1024,
            deadline,
        )?;
        decode_chunked(&encoded, max_response_bytes)?
    } else if let Some(length) = content_length {
        if length > max_response_bytes {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!("response of {length} bytes exceeds the {max_response_bytes} byte limit"),
            ));
        }
        let mut body = rest;
        while body.len() < length {
            let mut buffer = [0_u8; 8192];
            let count = read_some(stream, &mut buffer, deadline)?;
            if count == 0 {
                return Err(AiError::new(
                    AI_E_CONNECT,
                    "connection closed before the response body was complete",
                ));
            }
            body.extend_from_slice(&buffer[..count]);
        }
        body.truncate(length);
        body
    } else {
        let mut body = rest;
        read_to_end_bounded(stream, &mut body, max_response_bytes, deadline)?;
        body
    };
    let body = String::from_utf8(body)
        .map_err(|_| AiError::new(AI_E_PROVIDER, "HTTP response body is not valid UTF-8"))?;
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

type ResponseHead = (Vec<u8>, u16, Vec<(String, String)>);

fn read_headers(stream: &mut TcpStream, deadline: Instant) -> Result<ResponseHead, AiError> {
    let mut data = Vec::new();
    loop {
        if let Some(position) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_text = std::str::from_utf8(&data[..position]).map_err(|_| {
                AiError::new(AI_E_PROVIDER, "HTTP response headers are not valid UTF-8")
            })?;
            let mut lines = header_text.split("\r\n");
            let status_line = lines.next().ok_or_else(|| {
                AiError::new(AI_E_PROVIDER, "HTTP response is missing a status line")
            })?;
            let mut parts = status_line.split_whitespace();
            let version = parts.next().unwrap_or_default();
            if !version.starts_with("HTTP/") {
                return Err(AiError::new(
                    AI_E_PROVIDER,
                    "invalid HTTP response status line",
                ));
            }
            let status = parts
                .next()
                .and_then(|code| code.parse::<u16>().ok())
                .ok_or_else(|| AiError::new(AI_E_PROVIDER, "invalid HTTP status code"))?;
            let mut headers = Vec::new();
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                let (name, value) = line
                    .split_once(':')
                    .ok_or_else(|| AiError::new(AI_E_PROVIDER, "malformed HTTP response header"))?;
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
            return Ok((data[position + 4..].to_vec(), status, headers));
        }
        if data.len() > MAX_HEADER_BYTES {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!("response headers exceed the {MAX_HEADER_BYTES} byte limit"),
            ));
        }
        let mut buffer = [0_u8; 8192];
        let count = read_some(stream, &mut buffer, deadline)?;
        if count == 0 {
            return Err(AiError::new(
                AI_E_CONNECT,
                "connection closed before the response headers were complete",
            ));
        }
        data.extend_from_slice(&buffer[..count]);
    }
}

fn read_some(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> Result<usize, AiError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(AiError::new(AI_E_TIMEOUT, "HTTP request timed out"));
    }
    stream
        .set_read_timeout(Some(remaining.max(Duration::from_millis(1))))
        .map_err(|error| AiError::new(AI_E_CONNECT, format!("cannot set read timeout: {error}")))?;
    match stream.read(buffer) {
        Ok(count) => Ok(count),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Err(AiError::new(AI_E_TIMEOUT, "HTTP request timed out"))
        }
        Err(error) => Err(AiError::new(
            AI_E_CONNECT,
            format!("failed to read response: {error}"),
        )),
    }
}

fn read_to_end_bounded(
    stream: &mut TcpStream,
    output: &mut Vec<u8>,
    limit: usize,
    deadline: Instant,
) -> Result<(), AiError> {
    if output.len() > limit {
        return Err(AiError::new(
            AI_E_RESPONSE_TOO_LARGE,
            format!("response exceeds the {limit} byte limit"),
        ));
    }
    loop {
        let mut buffer = [0_u8; 8192];
        let count = read_some(stream, &mut buffer, deadline)?;
        if count == 0 {
            return Ok(());
        }
        output.extend_from_slice(&buffer[..count]);
        if output.len() > limit {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!("response exceeds the {limit} byte limit"),
            ));
        }
    }
}

fn decode_chunked(data: &[u8], max_response_bytes: usize) -> Result<Vec<u8>, AiError> {
    let mut position = 0;
    let mut output = Vec::new();
    loop {
        let line_end = data[position..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| AiError::new(AI_E_PROVIDER, "malformed chunked response"))?;
        let line = std::str::from_utf8(&data[position..position + line_end])
            .map_err(|_| AiError::new(AI_E_PROVIDER, "malformed chunk size"))?;
        let size_text = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| AiError::new(AI_E_PROVIDER, "malformed chunk size"))?;
        position += line_end + 2;
        if size == 0 {
            break;
        }
        if output.len().saturating_add(size) > max_response_bytes {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!("response exceeds the {max_response_bytes} byte limit"),
            ));
        }
        let end = position
            .checked_add(size)
            .ok_or_else(|| AiError::new(AI_E_PROVIDER, "chunk size overflow"))?;
        let terminator_end = end
            .checked_add(2)
            .ok_or_else(|| AiError::new(AI_E_PROVIDER, "chunk size overflow"))?;
        if terminator_end > data.len() {
            return Err(AiError::new(AI_E_PROVIDER, "truncated chunked response"));
        }
        output.extend_from_slice(&data[position..end]);
        if &data[end..end + 2] != b"\r\n" {
            return Err(AiError::new(AI_E_PROVIDER, "malformed chunk terminator"));
        }
        position = end + 2;
    }
    Ok(output)
}

fn unique_temp_file() -> Result<(PathBuf, File), AiError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    for attempt in 0..64 {
        let path = std::env::temp_dir().join(format!(
            "vantyx-ai-curl-{}-{stamp}-{attempt}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(AiError::new(
                    AI_E_IO,
                    format!("cannot create a temporary file for curl: {error}"),
                ));
            }
        }
    }
    Err(AiError::new(
        AI_E_IO,
        "cannot create a unique temporary file for curl",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    fn serve_once(response: Vec<u8>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            let mut content_length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
                request.push_str(&line);
            }
            let mut body = vec![0_u8; content_length];
            reader.read_exact(&mut body).unwrap();
            request.push_str(&String::from_utf8_lossy(&body));
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
            request
        });
        (format!("http://{address}/v1/chat/completions"), handle)
    }

    #[test]
    fn parses_http_and_https_urls() {
        let url = parse_url("http://127.0.0.1:11434/v1/chat?x=1").unwrap();
        assert_eq!(url.scheme, UrlScheme::Http);
        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 11434);
        assert_eq!(url.path, "/v1/chat?x=1");
        assert!(url.is_loopback());
        let url = parse_url("https://api.example.com").unwrap();
        assert_eq!(url.scheme, UrlScheme::Https);
        assert_eq!(url.port, 443);
        assert_eq!(url.path, "/");
    }

    #[test]
    fn rejects_unsafe_urls() {
        assert!(parse_url("ftp://example.com").is_err());
        assert!(parse_url("https://user:pass@example.com").is_err());
        assert!(parse_url("https://example.com/#fragment").is_err());
        assert!(parse_url("https://exa mple.com").is_err());
        assert!(parse_url("https://[::1]:443/").is_ok());
    }

    #[test]
    fn enforces_network_policy() {
        let local = parse_url("http://127.0.0.1:8080").unwrap();
        assert!(NetworkPolicy::local_only().check(&local).is_ok());
        let remote_http = parse_url("http://example.com").unwrap();
        assert_eq!(
            NetworkPolicy::with_network()
                .check(&remote_http)
                .unwrap_err()
                .code,
            AI_E_NETWORK_DISABLED
        );
        let remote_https = parse_url("https://example.com").unwrap();
        assert_eq!(
            NetworkPolicy::local_only()
                .check(&remote_https)
                .unwrap_err()
                .code,
            AI_E_NETWORK_DISABLED
        );
        assert!(
            NetworkPolicy::with_network()
                .with_allowed_host("example.com")
                .check(&remote_https)
                .is_ok()
        );
    }

    #[test]
    fn posts_to_a_loopback_server() {
        let body = r#"{"ok":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, handle) = serve_once(response.into_bytes());
        let transport = HttpTransport::new(NetworkPolicy::local_only());
        let response = transport
            .request(
                "POST",
                &url,
                &[("Content-Type".into(), "application/json".into())],
                Some(r#"{"prompt":"hi"}"#),
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, body);
        let request = handle.join().unwrap();
        assert!(request.contains("POST /v1/chat/completions HTTP/1.1"));
        assert!(request.contains(r#"{"prompt":"hi"}"#));
    }

    #[test]
    fn decodes_chunked_responses() {
        let (url, handle) = serve_once(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
                .to_vec(),
        );
        let transport = HttpTransport::new(NetworkPolicy::local_only());
        let response = transport.request("GET", &url, &[], None).unwrap();
        assert_eq!(response.body, "hello world");
        handle.join().unwrap();
    }

    #[test]
    fn rejects_oversized_responses() {
        let body = "x".repeat(100);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, handle) = serve_once(response.into_bytes());
        let transport = HttpTransport::new(NetworkPolicy::local_only()).with_max_response_bytes(10);
        assert_eq!(
            transport.request("GET", &url, &[], None).unwrap_err().code,
            AI_E_RESPONSE_TOO_LARGE
        );
        handle.join().unwrap();
    }

    #[test]
    fn rejects_header_injection() {
        let transport = HttpTransport::new(NetworkPolicy::local_only());
        let error = transport
            .request(
                "GET",
                "http://127.0.0.1:1/",
                &[("X-Test".into(), "a\r\nInjected: yes".into())],
                None,
            )
            .unwrap_err();
        assert_eq!(error.code, AI_E_CONFIG);
    }

    #[test]
    fn reports_connection_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let transport = HttpTransport::new(NetworkPolicy::local_only());
        let error = transport
            .request("GET", &format!("http://{address}/"), &[], None)
            .unwrap_err();
        assert_eq!(error.code, AI_E_CONNECT);
    }
}
