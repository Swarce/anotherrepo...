//! The server application: routes HTTP requests to the API (encrypted envelope) or the CDN,
//! logs one line per request, and keeps the counters for the `[status]` line.

use crate::{cdn, Ctx};
use jdr_core::envelope::{self, ResponseMeta};
use jdr_core::http::{Request, Response};
use jdr_core::log;
use serde_json::{Map, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

/// Path prefix of the API base we hand out in `get_version.url`.
pub const API_PREFIX: &str = "/api";
/// Path prefix of the CDN base we hand out in `get_version.s3_url`.
pub const CDN_PREFIX: &str = "/cdn";

/// Result of one API handler: the endpoint's `param` object (status defaults to OK).
pub type ApiResult = Result<Map<String, Value>, ApiError>;

#[derive(Debug)]
pub enum ApiError {
    /// A game-level status code (see ResponseParameterCode.Status).
    Status(i64),
    Internal(String),
}

pub fn handle(ctx: &Arc<Ctx>, req: &Request) -> Response {
    let t0 = Instant::now();
    ctx.counters.requests.fetch_add(1, Ordering::Relaxed);
    let (resp, tag, note) = route(ctx, req);
    ctx.counters
        .bytes_out
        .fetch_add(resp.body.len() as u64, Ordering::Relaxed);
    if resp.status >= 400 {
        ctx.counters.errors.fetch_add(1, Ordering::Relaxed);
    }
    log::line(
        tag,
        &format!(
            "{} {} {} in={}B out={}B {}ms{}",
            req.method,
            req.path,
            resp.status,
            req.body.len(),
            resp.body.len(),
            t0.elapsed().as_millis(),
            note
        ),
    );
    resp
}

fn route(ctx: &Arc<Ctx>, req: &Request) -> (Response, &'static str, String) {
    let path = req.path.as_str();
    if let Some(rel) = path.strip_prefix(CDN_PREFIX) {
        ctx.counters.cdn.fetch_add(1, Ordering::Relaxed);
        return match &ctx.settings.files_dir {
            Some(dir) => (cdn::serve_file(dir, rel, req), "cdn", String::new()),
            None => (
                Response::text(404, "no files dir configured"),
                "cdn",
                " (no --files)".into(),
            ),
        };
    }
    if let Some(rel) = path.strip_prefix(crate::web::WEB_PREFIX) {
        return (crate::web::page(rel), "web", String::new());
    }
    if path == "/" || path == "/status" {
        return (
            Response::text(
                200,
                &format!(
                    "JDR server build {}\n{}\n",
                    jdr_core::BUILD,
                    ctx.counters.status_line()
                ),
            ),
            "http",
            String::new(),
        );
    }
    if req.method != "POST" {
        return (Response::text(404, "not found"), "http", String::new());
    }
    // First call goes to the bare host (`/common/get_version`); later ones to `/api/...`.
    let api_name = path.strip_prefix(API_PREFIX).unwrap_or(path);
    api(ctx, api_name, req)
}

fn api(ctx: &Arc<Ctx>, api_name: &str, req: &Request) -> (Response, &'static str, String) {
    ctx.counters.api.fetch_add(1, Ordering::Relaxed);
    let Some(key) = ctx.http_key() else {
        return (
            Response::text(
                500,
                "http_key missing from keys.json (run jdr-tool keys --apk)",
            ),
            "api",
            String::new(),
        );
    };
    let env = match envelope::decode_request(&req.body, key) {
        Ok(e) => e,
        Err(e) => {
            return (
                Response::text(400, &e.to_string()),
                "api",
                format!(" decode error: {e}"),
            )
        }
    };
    if ctx.settings_trace() {
        log::line("trace", &format!("{api_name} <- {}", env.param));
    }
    let (result, known) = crate::services::dispatch(ctx, api_name, &env);
    if !known {
        ctx.counters.unknown.fetch_add(1, Ordering::Relaxed);
    }
    let (param, note) = match result {
        Ok(p) => (
            p,
            if known {
                String::new()
            } else {
                format!(
                    " UNKNOWN ROUTE param={}",
                    truncate(&env.param.to_string(), 300)
                )
            },
        ),
        Err(ApiError::Status(code)) => {
            let mut m = Map::new();
            m.insert("status".into(), code.into());
            (m, format!(" status={code}"))
        }
        Err(ApiError::Internal(e)) => {
            ctx.counters.errors.fetch_add(1, Ordering::Relaxed);
            let mut m = Map::new();
            m.insert("status".into(), 10001.into()); // E_NG -> client error dialog
            (m, format!(" INTERNAL ERROR {e}"))
        }
    };
    let session = ctx.session_for(&env);
    let meta = ResponseMeta {
        session,
        date: ctx.server_date(),
        datal_ver: ctx.datal_ver(),
        server_ver: env.version.clone(),
    };
    let json = envelope::response_json(param, &meta);
    if ctx.settings_trace() {
        log::line("trace", &format!("{api_name} -> {}", truncate(&json, 2000)));
    }
    let body = envelope::encode_response_json(&json, key, &envelope::next_iv());
    (Response::ok("text/plain", body), "api", note)
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
