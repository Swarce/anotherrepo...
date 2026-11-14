//! The game's request/response envelope (docs/protocol/envelope.md).
//!
//! Request:  `POST <base><apiName>`, body `data=` + pct(pct(base64(IV16 || AES-128-CBC-PKCS7(json))))`.
//! Response: HTTP 200, body `base64(IV16 || AES-128-CBC-PKCS7(json))`, same key both ways.
//! JSON is Unity `JsonUtility`: compact, field names as declared, enums as ints.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use jdr_crypto::aescbc;
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// `ResponseParameterCode.Status.E_OK`.
pub const STATUS_OK: i64 = 10000;

#[derive(Debug, Clone, PartialEq)]
pub struct RequestEnvelope {
    pub uuid: String,
    pub session: String,
    pub language: i64,
    /// Device-local `yyyyMMddHHmmss` as a number.
    pub date: i64,
    pub platform: i64,
    pub version: String,
    pub lz_accept: i64,
    pub lz_encode: i64,
    pub param: Value,
}

#[derive(Debug, PartialEq)]
pub enum EnvelopeError {
    NoDataField,
    Base64,
    Crypt(String),
    Json(String),
    Compressed,
}
impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeError::NoDataField => write!(f, "body has no data= field"),
            EnvelopeError::Base64 => write!(f, "data is not base64"),
            EnvelopeError::Crypt(e) => write!(f, "decrypt failed: {e}"),
            EnvelopeError::Json(e) => write!(f, "bad JSON: {e}"),
            EnvelopeError::Compressed => write!(
                f,
                "compressed request params are not supported (client never sends them)"
            ),
        }
    }
}
impl std::error::Error for EnvelopeError {}

fn pct_decode_all(mut s: String) -> String {
    // The client escapes twice; base64 never contains '%', so decode until none remain.
    for _ in 0..4 {
        if !s.contains('%') {
            break;
        }
        s = percent_decode(&s);
    }
    s
}

/// Percent-decoding without '+' -> ' ' (the inner layer is base64, which uses '+').
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..(i + 3).min(b.len())])
                .ok()
                .filter(|h| h.len() == 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn strip_ctrl(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r' | '\0' | '\x0b'))
        .collect()
}

pub fn decode_request(body: &[u8], key: &[u8]) -> Result<RequestEnvelope, EnvelopeError> {
    let text = std::str::from_utf8(body).map_err(|_| EnvelopeError::NoDataField)?;
    let data = text
        .split('&')
        .find_map(|kv| kv.strip_prefix("data="))
        .ok_or(EnvelopeError::NoDataField)?;
    let b64 = pct_decode_all(data.to_string());
    let raw = B64.decode(b64.trim()).map_err(|_| EnvelopeError::Base64)?;
    if raw.len() < 32 {
        return Err(EnvelopeError::Crypt("too short".into()));
    }
    let plain = aescbc::decrypt_pkcs7(key, &raw[..16], &raw[16..])
        .map_err(|e| EnvelopeError::Crypt(e.to_string()))?;
    let js = strip_ctrl(&String::from_utf8_lossy(&plain));
    let v: Value = serde_json::from_str(&js).map_err(|e| EnvelopeError::Json(e.to_string()))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    let env = RequestEnvelope {
        uuid: s("uuid"),
        session: s("session"),
        language: n("language"),
        date: n("date"),
        platform: n("platform"),
        version: s("version"),
        lz_accept: n("lzAccept"),
        lz_encode: n("lzEncode"),
        param: v.get("param").cloned().unwrap_or_else(|| json!({})),
    };
    if env.lz_encode == 1 && env.param.is_string() {
        return Err(EnvelopeError::Compressed);
    }
    Ok(env)
}

/// Top-level response fields besides `param`.
#[derive(Debug, Clone)]
pub struct ResponseMeta {
    pub session: String,
    /// `yyyy-MM-dd HH:mm:ss` server time (JST).
    pub date: String,
    pub datal_ver: String,
    pub server_ver: String,
}

/// Fill the fields every `ResponseParamBase` carries, unless the handler set them.
pub fn with_base_fields(mut param: Map<String, Value>) -> Map<String, Value> {
    param.entry("status").or_insert(json!(STATUS_OK));
    param
        .entry("emergencyInfo")
        .or_insert(json!({"id": 0, "info": "", "scroll": 0}));
    param.entry("frenzyTextId").or_insert(json!(""));
    param
}

pub fn response_json(param: Map<String, Value>, meta: &ResponseMeta) -> String {
    let env = json!({
        "session": meta.session,
        "result": 0,
        "date": meta.date,
        "datalVer": meta.datal_ver,
        "serverVer": meta.server_ver,
        "lzEncode": 0,
        "param": Value::Object(with_base_fields(param)),
    });
    serde_json::to_string(&env).expect("serializable")
}

static IV_COUNTER: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

/// 16 lowercase hex ASCII chars, like the client's Guid-derived IVs. Uniqueness, not secrecy,
/// is all that is needed here.
pub fn next_iv() -> [u8; 16] {
    let mut x = IV_COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
        ^ crate::log::unix_ms().rotate_left(17);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    let h = format!("{x:016x}");
    h.as_bytes().try_into().unwrap()
}

pub fn encode_response_json(json: &str, key: &[u8], iv: &[u8; 16]) -> Vec<u8> {
    let ct = aescbc::encrypt_pkcs7(key, iv, json.as_bytes()).expect("valid key");
    let mut buf = iv.to_vec();
    buf.extend_from_slice(&ct);
    B64.encode(buf).into_bytes()
}

pub fn encode_response(param: Map<String, Value>, meta: &ResponseMeta, key: &[u8]) -> Vec<u8> {
    encode_response_json(&response_json(param, meta), key, &next_iv())
}

/// Test helper / client mirror: build a request body exactly like the client does.
pub fn encode_request_like_client(json: &str, key: &[u8], iv: &[u8; 16]) -> Vec<u8> {
    let ct = aescbc::encrypt_pkcs7(key, iv, json.as_bytes()).expect("valid key");
    let mut buf = iv.to_vec();
    buf.extend_from_slice(&ct);
    let b64 = B64.encode(buf);
    let esc = |s: &str| -> String {
        s.bytes()
            .map(|c| {
                if c.is_ascii_alphanumeric() || b"-_.~".contains(&c) {
                    (c as char).to_string()
                } else {
                    format!("%{c:02X}")
                }
            })
            .collect()
    };
    format!("data={}", esc(&esc(&b64))).into_bytes()
}

/// Client mirror: decode a response body like `ResponseInfo.ResponseCallback`.
pub fn decode_response_like_client(body: &[u8], key: &[u8]) -> Result<Value, EnvelopeError> {
    let text = percent_decode(std::str::from_utf8(body).map_err(|_| EnvelopeError::Base64)?);
    let raw = B64.decode(text.trim()).map_err(|_| EnvelopeError::Base64)?;
    if raw.len() < 32 {
        return Err(EnvelopeError::Crypt("too short".into()));
    }
    let plain = aescbc::decrypt_pkcs7(key, &raw[..16], &raw[16..])
        .map_err(|e| EnvelopeError::Crypt(e.to_string()))?;
    serde_json::from_str(&strip_ctrl(&String::from_utf8_lossy(&plain)))
        .map_err(|e| EnvelopeError::Json(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    const K: &[u8; 16] = b"Synthetic-Key-16";

    #[test]
    fn request_round_trip_double_escaped() {
        let js = r#"{"uuid":"","session":"","language":1,"date":20261114103000,"platform":2,"version":"v","lzAccept":1,"lzEncode":0,"param":{"model":"SM-S901B","os":"Android OS 16","x":"a+b/c="}}"#;
        // pick an IV that produces '+' and '/' in base64 often enough; test several
        for i in 0..20u8 {
            let mut iv = *b"0123456789abcdef";
            iv[0] = b'a' + i;
            let body = encode_request_like_client(js, K, &iv);
            let r = decode_request(&body, K).unwrap();
            assert_eq!(r.platform, 2);
            assert_eq!(r.date, 20261114103000);
            assert_eq!(r.param["model"], "SM-S901B");
            assert_eq!(r.param["x"], "a+b/c=");
        }
    }

    #[test]
    fn response_round_trip_and_base_fields() {
        let meta = ResponseMeta {
            session: "s1".into(),
            date: "2026-11-14 10:30:00".into(),
            datal_ver: "1".into(),
            server_ver: "v".into(),
        };
        let mut p = Map::new();
        p.insert("url".into(), json!("http://127.0.0.1:18080"));
        p.insert("text".into(), json!("line1\nline2\t終"));
        let body = encode_response(p, &meta, K);
        assert!(body
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || b"+/=".contains(c)));
        let v = decode_response_like_client(&body, K).unwrap();
        assert_eq!(v["session"], "s1");
        assert_eq!(v["param"]["status"], 10000);
        assert_eq!(v["param"]["emergencyInfo"]["id"], 0);
        assert_eq!(
            v["param"]["text"], "line1\nline2\t終",
            "control chars survive because they are JSON-escaped"
        );
        assert_eq!(v["lzEncode"], 0);
    }

    #[test]
    fn handler_status_is_kept() {
        let mut p = Map::new();
        p.insert("status".into(), json!(50002));
        assert_eq!(with_base_fields(p)["status"], 50002);
    }

    #[test]
    fn errors() {
        assert_eq!(
            decode_request(b"foo=bar", K),
            Err(EnvelopeError::NoDataField)
        );
        assert_eq!(decode_request(b"data=!!!", K), Err(EnvelopeError::Base64));
        assert!(matches!(
            decode_request(b"data=AAAA", K),
            Err(EnvelopeError::Crypt(_))
        ));
    }

    #[test]
    fn ivs_are_hex_and_differ() {
        let a = next_iv();
        let b = next_iv();
        assert_ne!(a, b);
        assert!(a
            .iter()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn percent_decode_edges() {
        assert_eq!(percent_decode("%2B%2F%3D"), "+/=");
        assert_eq!(percent_decode("a%"), "a%");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a+b"), "a+b");
        assert_eq!(pct_decode_all("%252B".into()), "+");
    }
}
