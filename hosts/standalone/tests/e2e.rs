//! End to end over a real socket with synthetic keys: client-mirror request -> server -> decode.

use jdr_core::envelope::{decode_response_like_client, encode_request_like_client};
use jdr_core::http;
use jdr_core::settings::{Keys, Settings};
use jdr_services::{app, Ctx};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::Arc;

const HTTP_KEY: &str = "Synthetic-Key-16";

fn start(files: Option<std::path::PathBuf>) -> u16 {
    let mut s = Settings::new(std::env::temp_dir().join(format!("jdr-e2e-{}", std::process::id())));
    s.keys = Some(Keys {
        asset_key: "A".repeat(16),
        asset_iv: "B".repeat(16),
        sound_key: None,
        http_key: Some(HTTP_KEY.into()),
    });
    s.files_dir = files;
    let (l, port) = http::bind(&[0]).unwrap();
    let ctx = Arc::new(Ctx::new(s, false));
    ctx.port.store(port, Ordering::Relaxed);
    let h: http::Handler = Arc::new(move |r| app::handle(&ctx, r));
    std::thread::spawn(move || http::serve(l, h));
    port
}

fn post(port: u16, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nUser-Agent: jgr (Android; test)\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    let mut out = vec![];
    s.read_to_end(&mut out).unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status: u16 = std::str::from_utf8(&out[9..12]).unwrap().parse().unwrap();
    (status, out[split + 4..].to_vec())
}

fn call(port: u16, path: &str, param: &str) -> serde_json::Value {
    let js = format!(
        r#"{{"uuid":"","session":"","language":1,"date":20261114103000,"platform":2,"version":"vtest","lzAccept":1,"lzEncode":0,"param":{param}}}"#
    );
    let (st, body) = post(
        port,
        path,
        &encode_request_like_client(&js, HTTP_KEY.as_bytes(), b"0123456789abcdef"),
    );
    assert_eq!(st, 200);
    decode_response_like_client(&body, HTTP_KEY.as_bytes()).unwrap()
}

#[test]
fn get_version_hands_out_local_bases() {
    let port = start(None);
    let v = call(port, "/common/get_version", "{}");
    assert_eq!(v["param"]["status"], 10000);
    assert_eq!(v["param"]["url"], format!("http://127.0.0.1:{port}/api"));
    assert_eq!(v["param"]["s3_url"], format!("http://127.0.0.1:{port}/cdn"));
    assert!(!v["session"].as_str().unwrap().is_empty());
    assert_eq!(v["date"].as_str().unwrap().len(), 19);
}

#[test]
fn unknown_route_answers_ok() {
    let port = start(None);
    let v = call(port, "/api/nope/nothing", r#"{"x":1}"#);
    assert_eq!(v["param"]["status"], 10000);
}

#[test]
fn garbage_body_is_400_not_a_crash() {
    let port = start(None);
    assert_eq!(post(port, "/api/user/login", b"data=%%%").0, 400);
    assert_eq!(post(port, "/api/user/login", b"").0, 400);
}

#[test]
fn cdn_serves_files() {
    let d = std::env::temp_dir().join(format!("jdr-e2e-files-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("abc.bytes"), b"bundle").unwrap();
    let port = start(Some(d));
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"GET /cdn/abc.bytes HTTP/1.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(
        out.starts_with("HTTP/1.1 200") && out.ends_with("bundle"),
        "{out}"
    );
}
