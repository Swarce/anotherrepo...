//! A small HTTP/1.1 server on std only (no async runtime), so the same code runs inside the
//! game process. Binds 127.0.0.1 only. One thread per connection, keep-alive supported.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Concurrent connections served; extra connections get 503 and are closed.
pub const MAX_CONNECTIONS: usize = 64;

static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Read one line of at most `limit` bytes. Errors with 400 if the line is longer.
fn read_line_limited<R: BufRead>(
    r: &mut R,
    out: &mut String,
    limit: usize,
) -> Result<usize, ReadError> {
    use std::io::Read as _;
    let n = r.by_ref().take(limit as u64).read_line(out)?;
    if n == limit && !out.ends_with('\n') {
        return Err(ReadError::Bad(400, "line too long"));
    }
    Ok(n)
}

/// Read exactly `n` bytes, growing the buffer only as data arrives.
fn read_body<R: BufRead>(r: &mut R, body: &mut Vec<u8>, n: usize) -> Result<(), ReadError> {
    use std::io::Read as _;
    let got = r.by_ref().take(n as u64).read_to_end(body)?;
    if got != n {
        return Err(ReadError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "truncated body",
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub method: String,
    /// Path without the query string, percent-decoding NOT applied.
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), content_type.into())],
            body,
        }
    }
    pub fn ok(content_type: &str, body: Vec<u8>) -> Self {
        Self::new(200, content_type, body)
    }
    pub fn text(status: u16, s: &str) -> Self {
        Self::new(status, "text/plain; charset=utf-8", s.as_bytes().to_vec())
    }
}

pub type Handler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        416 => "Range Not Satisfiable",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

/// Bind the first free port of `ports` on 127.0.0.1.
pub fn bind(ports: &[u16]) -> std::io::Result<(TcpListener, u16)> {
    let mut last = None;
    for &p in ports {
        match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], p))) {
            Ok(l) => {
                let port = l.local_addr()?.port();
                return Ok((l, port));
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last
        .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no ports given")))
}

/// Accept loop; returns only on listener error.
pub fn serve(listener: TcpListener, handler: Handler) -> std::io::Result<()> {
    use std::sync::atomic::Ordering;
    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        if ACTIVE.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
            let _ = write_response(&mut stream, &Response::text(503, "busy"), false, false);
            continue;
        }
        ACTIVE.fetch_add(1, Ordering::Relaxed);
        let h = handler.clone();
        let spawned = std::thread::Builder::new()
            .name("jdr-http".into())
            .spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_connection(stream, h)
                }));
                ACTIVE.fetch_sub(1, Ordering::Relaxed);
            });
        if spawned.is_err() {
            ACTIVE.fetch_sub(1, Ordering::Relaxed);
        }
    }
    Ok(())
}

pub fn handle_connection(stream: TcpStream, handler: Handler) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    stream.set_nodelay(true)?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    loop {
        let req = match read_request(&mut reader) {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(()), // clean EOF
            Err(ReadError::Io(e)) => return Err(e),
            Err(ReadError::Bad(code, msg)) => {
                let _ = write_response(&mut writer, &Response::text(code, msg), false, false);
                return Ok(());
            }
        };
        let keep_alive = !req
            .header("Connection")
            .map(|v| v.eq_ignore_ascii_case("close"))
            .unwrap_or(false);
        let head_only = req.method == "HEAD";
        let resp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&req)))
            .unwrap_or_else(|_| Response::text(500, "internal error"));
        write_response(&mut writer, &resp, keep_alive, head_only)?;
        if !keep_alive {
            return Ok(());
        }
    }
}

#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    Bad(u16, &'static str),
}

impl From<std::io::Error> for ReadError {
    fn from(e: std::io::Error) -> Self {
        ReadError::Io(e)
    }
}

pub fn read_request<R: BufRead>(r: &mut R) -> Result<Option<Request>, ReadError> {
    let mut line = String::new();
    // skip stray CRLFs between requests
    for _ in 0..8 {
        line.clear();
        if read_line_limited(r, &mut line, MAX_HEADER_BYTES)? == 0 {
            return Ok(None);
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    if line.trim().is_empty() {
        return Err(ReadError::Bad(400, "no request line"));
    }
    let mut parts = line.split_whitespace();
    let method = parts
        .next()
        .ok_or(ReadError::Bad(400, "bad request line"))?
        .to_string();
    let target = parts
        .next()
        .ok_or(ReadError::Bad(400, "bad request line"))?;
    let target = target
        .strip_prefix("http://")
        .map(|t| t.find('/').map(|i| &t[i..]).unwrap_or("/"))
        .unwrap_or(target);
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    let mut headers = vec![];
    let mut total = line.len();
    loop {
        let mut h = String::new();
        if read_line_limited(r, &mut h, MAX_HEADER_BYTES)? == 0 {
            return Err(ReadError::Bad(400, "truncated headers"));
        }
        total += h.len();
        if total > MAX_HEADER_BYTES {
            return Err(ReadError::Bad(400, "headers too large"));
        }
        let h = h.trim_end_matches(['\r', '\n']);
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut req = Request {
        method,
        path,
        query,
        headers,
        body: vec![],
    };
    let chunked = req
        .header("Transfer-Encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    if chunked {
        loop {
            let mut sz = String::new();
            read_line_limited(r, &mut sz, 256)?;
            let n = usize::from_str_radix(sz.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| ReadError::Bad(400, "bad chunk size"))?;
            match req.body.len().checked_add(n) {
                Some(t) if t <= MAX_BODY_BYTES => {}
                _ => return Err(ReadError::Bad(413, "body too large")),
            }
            read_body(r, &mut req.body, n)?;
            let mut crlf = String::new();
            read_line_limited(r, &mut crlf, 256)?;
            if n == 0 {
                break;
            }
        }
    } else if let Some(cl) = req.header("Content-Length") {
        let n: usize = cl
            .parse()
            .map_err(|_| ReadError::Bad(400, "bad content-length"))?;
        if n > MAX_BODY_BYTES {
            return Err(ReadError::Bad(413, "body too large"));
        }
        read_body(r, &mut req.body, n)?;
    }
    Ok(Some(req))
}

pub fn write_response<W: Write>(
    w: &mut W,
    resp: &Response,
    keep_alive: bool,
    head_only: bool,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", resp.status, reason(resp.status));
    for (k, v) in &resp.headers {
        head += &format!("{k}: {v}\r\n");
    }
    head += &format!("Content-Length: {}\r\n", resp.body.len());
    head += if keep_alive {
        "Connection: keep-alive\r\n\r\n"
    } else {
        "Connection: close\r\n\r\n"
    };
    w.write_all(head.as_bytes())?;
    if !head_only {
        w.write_all(&resp.body)?;
    }
    w.flush()
}

/// Minimal `application/x-www-form-urlencoded` parsing.
pub fn parse_form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|s| !s.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (url_decode(k), url_decode(v))
        })
        .collect()
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|x| u8::from_str_radix(x, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn parses_post_with_body_and_keepalive_sequence() {
        let raw = b"POST /api/common/get_version?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 5\r\n\r\nhelloGET /b HTTP/1.1\r\n\r\n";
        let mut c = Cursor::new(&raw[..]);
        let r = read_request(&mut c).unwrap().unwrap();
        assert_eq!(
            (r.method.as_str(), r.path.as_str(), r.query.as_str()),
            ("POST", "/api/common/get_version", "x=1")
        );
        assert_eq!(r.body, b"hello");
        assert_eq!(r.header("host"), Some("127.0.0.1"));
        let r2 = read_request(&mut c).unwrap().unwrap();
        assert_eq!(r2.path, "/b");
        assert!(read_request(&mut c).unwrap().is_none());
    }

    #[test]
    fn parses_chunked_and_absolute_uri() {
        let raw = b"POST http://127.0.0.1:18080/x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        let r = read_request(&mut Cursor::new(&raw[..])).unwrap().unwrap();
        assert_eq!(r.path, "/x");
        assert_eq!(r.body, b"abcde");
    }

    #[test]
    fn rejects_huge_body() {
        let raw = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(matches!(
            read_request(&mut Cursor::new(raw.as_bytes())),
            Err(ReadError::Bad(413, _))
        ));
    }

    #[test]
    fn rejects_endless_line_and_huge_chunk() {
        let long = vec![b'A'; MAX_HEADER_BYTES + 10];
        assert!(matches!(
            read_request(&mut Cursor::new(&long[..])),
            Err(ReadError::Bad(400, _))
        ));
        let raw = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n";
        assert!(matches!(
            read_request(&mut Cursor::new(&raw[..])),
            Err(ReadError::Bad(413, _))
        ));
        let short = b"POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc";
        assert!(matches!(
            read_request(&mut Cursor::new(&short[..])),
            Err(ReadError::Io(_))
        ));
    }

    #[test]
    fn form_decoding() {
        let f = parse_form("data=a%2Bb%3D%3D&x=1+2&bad=%zz&tail=%4");
        assert_eq!(f[0], ("data".into(), "a+b==".into()));
        assert_eq!(f[1].1, "1 2");
        assert_eq!(f[2].1, "%zz");
        assert_eq!(f[3].1, "%4");
    }

    #[test]
    fn round_trip_over_socket() {
        let (l, port) = bind(&[0]).unwrap();
        let h: Handler = Arc::new(|r: &Request| {
            Response::ok(
                "text/plain",
                format!("{} {}", r.method, r.body.len()).into_bytes(),
            )
        });
        std::thread::spawn(move || serve(l, h));
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(b"POST /a HTTP/1.1\r\nContent-Length: 3\r\n\r\nxyzGET /b HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.contains("POST 3"), "{out}");
        assert!(out.contains("GET 0"), "{out}");
    }
}
