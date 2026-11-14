//! Asset CDN: serves files from the player's own copy of the game's downloaded data.
//! Never downloads from, or proxies to, any third party.

use jdr_core::http::{Request, Response};
use std::path::{Component, Path, PathBuf};

/// Resolve `rel` inside `root`, refusing anything that could escape it.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim_start_matches('/');
    if rel.is_empty() || rel.contains('\\') || rel.contains('\0') {
        return None;
    }
    let p = Path::new(rel);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(root.join(p))
}

/// Parse `Range: bytes=a-b` (single range only).
pub fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let spec = h.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return None;
        }
        (len.saturating_sub(n), len.checked_sub(1)?)
    } else {
        let s: u64 = a.parse().ok()?;
        let e: u64 = if b.is_empty() {
            len.checked_sub(1)?
        } else {
            b.parse::<u64>().ok()?.min(len.checked_sub(1)?)
        };
        (s, e)
    };
    (start <= end && end < len).then_some((start, end))
}

/// `bytes=a-b` / `bytes=a-` / `bytes=-n` with a <= b; anything else is ignored.
pub fn range_is_well_formed(h: &str) -> bool {
    let Some(spec) = h.trim().strip_prefix("bytes=") else {
        return false;
    };
    let Some((a, b)) = spec.split_once('-') else {
        return false;
    };
    let num = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    match (a.is_empty(), b.is_empty()) {
        (false, true) => num(a),
        (true, false) => num(b),
        (false, false) => num(a) && num(b) && a.parse::<u64>().ok() <= b.parse::<u64>().ok(),
        (true, true) => false,
    }
}

/// Serve `rel` from `root` (GET/HEAD, optional Range).
pub fn serve_file(root: &Path, rel: &str, req: &Request) -> Response {
    let Some(path) = safe_join(root, rel) else {
        return Response::text(400, "bad path");
    };
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(_) => return Response::text(404, "not found"),
    };
    let len = data.len() as u64;
    // RFC 7233: ignore malformed or non-byte ranges; 416 only for a well-formed range that
    // starts past the end of the file.
    if let Some(r) = req.header("Range").filter(|r| range_is_well_formed(r)) {
        return match parse_range(r, len) {
            Some((s, e)) => {
                let mut resp = Response::new(
                    206,
                    "application/octet-stream",
                    data[s as usize..=e as usize].to_vec(),
                );
                resp.headers
                    .push(("Content-Range".into(), format!("bytes {s}-{e}/{len}")));
                resp
            }
            None => {
                let mut resp = Response::text(416, "range not satisfiable");
                resp.headers
                    .push(("Content-Range".into(), format!("bytes */{len}")));
                resp
            }
        };
    }
    let mut resp = Response::ok("application/octet-stream", data);
    resp.headers.push(("Accept-Ranges".into(), "bytes".into()));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety() {
        let r = Path::new("/srv/files");
        assert_eq!(
            safe_join(r, "abc.bytes"),
            Some(PathBuf::from("/srv/files/abc.bytes"))
        );
        assert_eq!(
            safe_join(r, "/sub/abc.bytes"),
            Some(PathBuf::from("/srv/files/sub/abc.bytes"))
        );
        for bad in ["../x", "a/../../x", "", "a\\b", "./a", "/"] {
            assert_eq!(safe_join(r, bad), None, "{bad}");
        }
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
        assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=50-500", 100), Some((50, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 100), None);
        assert_eq!(parse_range("bytes=-0", 100), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    #[test]
    fn serves_with_range() {
        let d = std::env::temp_dir().join(format!("jdr-cdn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("f.bytes"), b"0123456789").unwrap();
        let mut req = Request {
            method: "GET".into(),
            ..Default::default()
        };
        assert_eq!(serve_file(&d, "f.bytes", &req).body, b"0123456789");
        req.headers.push(("Range".into(), "bytes=2-4".into()));
        let r = serve_file(&d, "f.bytes", &req);
        assert_eq!((r.status, r.body.as_slice()), (206, b"234".as_slice()));
        assert_eq!(serve_file(&d, "missing", &req).status, 404);
        req.headers = vec![("Range".into(), "bytes=5-3".into())];
        assert_eq!(
            serve_file(&d, "f.bytes", &req).status,
            200,
            "reversed range is ignored"
        );
        req.headers = vec![("Range".into(), "items=0-1".into())];
        assert_eq!(
            serve_file(&d, "f.bytes", &req).status,
            200,
            "non-byte unit is ignored"
        );
        req.headers = vec![("Range".into(), "bytes=50-".into())];
        assert_eq!(serve_file(&d, "f.bytes", &req).status, 416);
        assert_eq!(serve_file(&d, "../etc/passwd", &req).status, 400);
    }
}
