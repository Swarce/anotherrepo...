//! Web pages the client opens in a WebView under `get_version.s3_url` (terms, help, notices).
//! The terms page matters: the Accept button only appears if it loads (new players only).

use jdr_core::http::Response;

pub const WEB_PREFIX: &str = "/web";

const STYLE: &str = "body{font-family:sans-serif;background:#1b1426;color:#f3eefc;margin:24px;line-height:1.5}h1{font-size:1.4em}";

fn html(title: &str, body: &str) -> Response {
    let s = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title><style>{STYLE}</style></head><body><h1>{title}</h1>{body}</body></html>"
    );
    Response::ok("text/html; charset=utf-8", s.into_bytes())
}

pub fn page(rel: &str) -> Response {
    match rel {
        "/information/terms_a.html" => html(
            "Offline preservation build",
            "<p>This copy of the game runs entirely on your device. There is no online service, no account on any server, no purchases and no tracking.</p><p>Tap <b>Accept</b> to continue.</p>",
        ),
        "/information/copyright.html" => html(
            "Credits",
            "<p>Unofficial, non-commercial preservation build. All rights to the game and to JoJo's Bizarre Adventure belong to their respective owners.</p>",
        ),
        _ => html("Offline", "<p>This page was part of the online service and is not available offline.</p>"),
    }
}
