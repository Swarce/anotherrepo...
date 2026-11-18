//! The Town home's realtime socket (docs/research/24-town.md §1.5), on the HTTP port: the client
//! connects to the host and port `/clan/sync_server_info` names and the HTTP server hands the
//! connection here when it starts with the frame magic.
//!
//! Wire format (H, 3.12.0): a 26-byte little-endian header ("GLPH", int64 version, int16
//! platform, int32 body size, int32 protocol id, int32 user id), then the body: the message as a
//! MsgPack map of field name to value (the old spec the client's ObjectPacker reads: raw strings,
//! float32 for Single), XOR the ASCII crypt key. Responses carry `result 0, status 10000`,
//! notices `status 10000`; any other status is shown as an error and may close the Town.
//!
//! Offline the only human is the player; the other members are AI: they are online in the
//! login notice, and now and then one walks to a spot the player has walked (so they stay on the
//! map) or does its action.

use crate::accounts::Change;
use crate::town::{self, Me, Town};
use crate::Ctx;
use jdr_core::log;
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MAGIC: [u8; 4] = *b"GLPH";
pub const HEADER_SIZE: usize = 26;
/// Our crypt key (ours; the client XORs with whatever `sync_server_info` names).
pub const CRYPT_KEY: &str = "jdrtown";
/// Largest body we accept (ours).
const MAX_BODY: usize = 256 * 1024;

/// `Jjs.Socket.PRTID` (client).
pub const REQ_LOGIN: i32 = 1000;
pub const REQ_CHAT_SEND: i32 = 1001;
pub const REQ_CHAR_MOVE: i32 = 1002;
pub const REQ_LOGOUT: i32 = 1003;
pub const REQ_CHAR_ACTION: i32 = 1004;
pub const REQ_CHAT_READED: i32 = 1005;
pub const REQ_SET_CLAN_OBJECT: i32 = 1006;
pub const REQ_SET_CLAN_BG: i32 = 1007;
pub const REQ_HEART_BEAT: i32 = 5000;
pub const RES_OFFSET: i32 = 10000;
pub const NOTICE_LOGIN: i32 = 30000;
pub const NOTICE_CHAT_SEND: i32 = 30001;
pub const NOTICE_CHAR_MOVE: i32 = 30002;
pub const NOTICE_CHAR_ACTION: i32 = 30004;
pub const NOTICE_SET_CLAN_OBJECT: i32 = 30006;
pub const NOTICE_SET_CLAN_BG: i32 = 30007;
/// `SocketStatus.Status` E_OK / E_CONNECT_CLAN (client).
pub const E_OK: i64 = 10000;
pub const E_CONNECT_CLAN: i64 = 20001;

/// How often an AI member moves or acts while the player is in the Town home (ours).
const AI_INTERVAL: Duration = Duration::from_secs(4);

// ---------------------------------------------------------------------------------------------
// MsgPack (the subset both sides use)

pub fn pack(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push(0xc0),
        Value::Bool(b) => out.push(if *b { 0xc3 } else { 0xc2 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64().filter(|_| !n.is_f64()) {
                pack_int(i, out);
            } else if let Some(u) = n.as_u64().filter(|_| !n.is_f64()) {
                pack_int(u.min(i64::MAX as u64) as i64, out);
            } else {
                out.push(0xca);
                out.extend_from_slice(&(n.as_f64().unwrap_or(0.0) as f32).to_be_bytes());
            }
        }
        Value::String(s) => {
            let b = s.as_bytes();
            match b.len() {
                n if n < 32 => out.push(0xa0 | n as u8),
                n if n < 0x1_0000 => {
                    out.push(0xda);
                    out.extend_from_slice(&(n as u16).to_be_bytes());
                }
                n => {
                    out.push(0xdb);
                    out.extend_from_slice(&(n as u32).to_be_bytes());
                }
            }
            out.extend_from_slice(b);
        }
        Value::Array(a) => {
            match a.len() {
                n if n < 16 => out.push(0x90 | n as u8),
                n if n < 0x1_0000 => {
                    out.push(0xdc);
                    out.extend_from_slice(&(n as u16).to_be_bytes());
                }
                n => {
                    out.push(0xdd);
                    out.extend_from_slice(&(n as u32).to_be_bytes());
                }
            }
            for x in a {
                pack(x, out);
            }
        }
        Value::Object(m) => {
            match m.len() {
                n if n < 16 => out.push(0x80 | n as u8),
                n if n < 0x1_0000 => {
                    out.push(0xde);
                    out.extend_from_slice(&(n as u16).to_be_bytes());
                }
                n => {
                    out.push(0xdf);
                    out.extend_from_slice(&(n as u32).to_be_bytes());
                }
            }
            for (k, x) in m {
                pack(&Value::String(k.clone()), out);
                pack(x, out);
            }
        }
    }
}

/// Integers in the forms the client's Int32 and UInt32 fields both accept (fixnum, uint8-32;
/// int8-32 when negative).
fn pack_int(i: i64, out: &mut Vec<u8>) {
    match i {
        0..=0x7f => out.push(i as u8),
        0x80..=0xff => out.extend_from_slice(&[0xcc, i as u8]),
        0x100..=0xffff => {
            out.push(0xcd);
            out.extend_from_slice(&(i as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(0xce);
            out.extend_from_slice(&(i as u32).to_be_bytes());
        }
        -32..=-1 => out.push(i as i8 as u8),
        -128..=-33 => out.extend_from_slice(&[0xd0, i as i8 as u8]),
        -32768..=-129 => {
            out.push(0xd1);
            out.extend_from_slice(&(i as i16).to_be_bytes());
        }
        _ if i >= i32::MIN as i64 && i < 0 => {
            out.push(0xd2);
            out.extend_from_slice(&(i as i32).to_be_bytes());
        }
        _ => {
            out.push(if i < 0 { 0xd3 } else { 0xcf });
            out.extend_from_slice(&i.to_be_bytes());
        }
    }
}

pub fn unpack(b: &[u8]) -> Option<Value> {
    let mut at = 0;
    let v = unpack_at(b, &mut at, 0)?;
    Some(v)
}

fn take<'a>(b: &'a [u8], at: &mut usize, n: usize) -> Option<&'a [u8]> {
    let s = b.get(*at..at.checked_add(n)?)?;
    *at += n;
    Some(s)
}

fn be(b: &[u8], at: &mut usize, n: usize) -> Option<u64> {
    Some(
        take(b, at, n)?
            .iter()
            .fold(0u64, |a, x| (a << 8) | *x as u64),
    )
}

fn unpack_at(b: &[u8], at: &mut usize, depth: usize) -> Option<Value> {
    if depth > 32 {
        return None;
    }
    let t = *take(b, at, 1)?.first()?;
    let raw = |at: &mut usize, n: usize| -> Option<Value> {
        Some(Value::String(
            String::from_utf8_lossy(take(b, at, n)?).into_owned(),
        ))
    };
    let arr = |at: &mut usize, n: usize| -> Option<Value> {
        (0..n)
            .map(|_| unpack_at(b, at, depth + 1))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array)
    };
    let map = |at: &mut usize, n: usize| -> Option<Value> {
        let mut m = Map::new();
        for _ in 0..n {
            let k = match unpack_at(b, at, depth + 1)? {
                Value::String(s) => s,
                other => other.to_string(),
            };
            let v = unpack_at(b, at, depth + 1)?;
            m.insert(k, v);
        }
        Some(Value::Object(m))
    };
    Some(match t {
        0x00..=0x7f => json!(t),
        0x80..=0x8f => map(at, (t & 0x0f) as usize)?,
        0x90..=0x9f => arr(at, (t & 0x0f) as usize)?,
        0xa0..=0xbf => raw(at, (t & 0x1f) as usize)?,
        0xc0 => Value::Null,
        0xc2 => json!(false),
        0xc3 => json!(true),
        0xc4 | 0xd9 => {
            let n = be(b, at, 1)? as usize;
            raw(at, n)?
        }
        0xc5 | 0xda => {
            let n = be(b, at, 2)? as usize;
            raw(at, n)?
        }
        0xc6 | 0xdb => {
            let n = be(b, at, 4)? as usize;
            raw(at, n)?
        }
        0xca => json!(f32::from_bits(be(b, at, 4)? as u32) as f64),
        0xcb => json!(f64::from_bits(be(b, at, 8)?)),
        0xcc => json!(be(b, at, 1)?),
        0xcd => json!(be(b, at, 2)?),
        0xce => json!(be(b, at, 4)?),
        0xcf => json!(be(b, at, 8)?),
        0xd0 => json!(be(b, at, 1)? as u8 as i8),
        0xd1 => json!(be(b, at, 2)? as u16 as i16),
        0xd2 => json!(be(b, at, 4)? as u32 as i32),
        0xd3 => json!(be(b, at, 8)? as i64),
        0xdc => {
            let n = be(b, at, 2)? as usize;
            arr(at, n)?
        }
        0xdd => {
            let n = be(b, at, 4)? as usize;
            arr(at, n)?
        }
        0xde => {
            let n = be(b, at, 2)? as usize;
            map(at, n)?
        }
        0xdf => {
            let n = be(b, at, 4)? as usize;
            map(at, n)?
        }
        0xe0..=0xff => json!(t as i8),
        _ => return None,
    })
}

pub fn xor(b: &mut [u8], key: &str) {
    let k = if key.is_empty() { "dummy" } else { key }.as_bytes();
    for (i, x) in b.iter_mut().enumerate() {
        *x ^= k[i % k.len()];
    }
}

// ---------------------------------------------------------------------------------------------
// Frames

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Header {
    pub version: i64,
    pub platform: i16,
    pub size: i32,
    pub pid: i32,
    pub user_id: i32,
}

pub fn encode_frame(h: Header, pid: i32, msg: &Value) -> Vec<u8> {
    let mut body = Vec::new();
    pack(msg, &mut body);
    xor(&mut body, CRYPT_KEY);
    let mut out = Vec::with_capacity(HEADER_SIZE + body.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&h.version.to_le_bytes());
    out.extend_from_slice(&h.platform.to_le_bytes());
    out.extend_from_slice(&(body.len() as i32).to_le_bytes());
    out.extend_from_slice(&pid.to_le_bytes());
    out.extend_from_slice(&h.user_id.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// Take one complete frame off the front of `buf`: its header and decoded message. `Err` when
/// the stream cannot be read (bad magic, oversized body).
#[allow(clippy::type_complexity)]
pub fn take_frame(buf: &mut Vec<u8>) -> Result<Option<(Header, Value)>, String> {
    if buf.len() < HEADER_SIZE {
        return Ok(None);
    }
    if buf[..4] != MAGIC {
        return Err("bad magic".into());
    }
    let i32le = |o: usize| i32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
    let h = Header {
        version: i64::from_le_bytes(buf[4..12].try_into().unwrap_or([0; 8])),
        platform: i16::from_le_bytes([buf[12], buf[13]]),
        size: i32le(14),
        pid: i32le(18),
        user_id: i32le(22),
    };
    let size = usize::try_from(h.size).map_err(|_| "negative size")?;
    if size > MAX_BODY {
        return Err(format!("body too large ({size})"));
    }
    if buf.len() < HEADER_SIZE + size {
        return Ok(None);
    }
    let mut body: Vec<u8> = buf[HEADER_SIZE..HEADER_SIZE + size].to_vec();
    buf.drain(..HEADER_SIZE + size);
    xor(&mut body, CRYPT_KEY);
    Ok(Some((h, unpack(&body).unwrap_or(Value::Null))))
}

// ---------------------------------------------------------------------------------------------
// Messages

fn ok_response() -> Value {
    json!({ "result": 0, "status": E_OK })
}

fn f(x: f64) -> Value {
    // Always a float (the client's Single fields read only float32).
    json!(x as f32 as f64 + 0.0)
}

fn pos3(p: [f64; 3]) -> (Value, Value, Value) {
    (f(p[0]), f(p[1]), f(p[2]))
}

/// `LoginNotify`: everybody in the Town (the AI members are online), objects, background, chat.
pub fn login_notify(t: &Town, me: &Me, now: i64) -> Value {
    let (x, y, z) = pos3(t.my_pos);
    let mut members = vec![json!({
        "user_id": me.id, "statue_id": me.statue_id, "model_list": me.models,
        "x": x, "y": y, "z": z, "user_name": me.name, "vampire_flg": false,
        "comment": me.comment, "position": t.position_of(&me.id)
    })];
    for m in &t.members {
        let (x, y, z) = pos3(m.pos);
        members.push(json!({
            "user_id": m.id, "statue_id": m.statue,
            "model_list": crate::collection::find(&m.statue).map(|d| d.model_data_list.clone()).unwrap_or_default(),
            "x": x, "y": y, "z": z, "user_name": m.name, "vampire_flg": false,
            "comment": "", "position": t.position_of(&m.id)
        }));
    }
    let _ = now;
    json!({
        "status": E_OK, "member": members, "leader_user_id": t.leader,
        "last_leader_user_id": t.leader, "clan_object": objects_list(t),
        "background_id": t.background,
        "chat_log": t.chat.iter().map(|c| json!({
            "user_id": c.user, "message": c.message, "message_at": town::date(c.at), "seq_id": c.seq
        })).collect::<Vec<_>>(),
        "last_seq_id": t.read_seq
    })
}

fn objects_list(t: &Town) -> Vec<Value> {
    t.placed
        .iter()
        .map(|[o, s]| json!({ "object_id": o, "slot_index": s }))
        .collect()
}

pub fn chat_notify(c: &town::ChatLine) -> Value {
    json!({ "status": E_OK, "user_id": c.user, "message": c.message,
            "message_at": town::date(c.at), "seq_id": c.seq })
}

pub fn move_notify(user: &str, p: [f64; 3]) -> Value {
    let (x, y, z) = pos3(p);
    json!({ "status": E_OK, "user_id": user, "x": x, "y": y, "z": z })
}

pub fn action_notify(user: &str) -> Value {
    json!({ "status": E_OK, "user_id": user })
}

pub fn set_object_notify(t: &Town) -> Value {
    json!({ "status": E_OK, "clan_object": objects_list(t) })
}

pub fn set_bg_notify(t: &Town, me: &Me) -> Value {
    let (x, y, z) = pos3(t.my_pos);
    let mut members = vec![json!({ "user_id": me.id, "x": x, "y": y, "z": z })];
    for m in &t.members {
        let (x, y, z) = pos3(m.pos);
        members.push(json!({ "user_id": m.id, "x": x, "y": y, "z": z }));
    }
    json!({ "status": E_OK, "member": members, "clan_object": objects_list(t),
            "background_id": t.background })
}

fn pos_of(v: &Value) -> [f64; 3] {
    let g = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let p = [g("x"), g("y"), g("z")];
    if p.iter().all(|c| c.is_finite() && c.abs() < 10_000.0) {
        p
    } else {
        [0.0; 3]
    }
}

// ---------------------------------------------------------------------------------------------
// The session

/// What one request does: the replies (protocol id, message) and whether to close.
pub struct Outcome {
    pub replies: Vec<(i32, Value)>,
    pub close: bool,
}

/// The session: its account (the login names it), and where the player walked (kept in memory:
/// moves come about once a second; the last position is saved when the socket closes).
#[derive(Default)]
pub struct Session {
    pub uuid: Option<String>,
    pub walked: Vec<[f64; 3]>,
    pub my_pos: Option<[f64; 3]>,
}

impl Session {
    /// Remember a position more than a metre from every one kept (AI members walk only there).
    pub fn walk(&mut self, pos: [f64; 3]) {
        self.my_pos = Some(pos);
        let far = self.walked.iter().all(|w| {
            let (dx, dz) = (w[0] - pos[0], w[2] - pos[2]);
            dx * dx + dz * dz > 1.0
        });
        if far {
            self.walked.push(pos);
            if self.walked.len() > town::WALKED_KEEP {
                self.walked.remove(0);
            }
        }
    }

    /// Save the player's last position.
    pub fn save(&mut self, ctx: &Ctx) {
        let (Some(uuid), Some(pos)) = (&self.uuid, self.my_pos.take()) else {
            return;
        };
        let _ = ctx
            .accounts
            .update(uuid, |p| match p.progress.town.town.as_mut() {
                Some(t) if t.my_pos != pos => {
                    t.my_pos = pos;
                    ((), Change::Bookkeeping)
                }
                _ => ((), Change::None),
            });
    }
}

/// Handle one request. State changes go through the account lock, like the HTTP routes.
pub fn handle(ctx: &Ctx, s: &mut Session, pid: i32, msg: &Value) -> Outcome {
    let now = jdr_core::clock::now();
    let mut replies = Vec::new();
    let mut close = false;
    let res = pid + RES_OFFSET;
    match pid {
        REQ_LOGIN => {
            let uuid = msg.get("uu_id").and_then(Value::as_str).unwrap_or("").to_string();
            let r = ctx.accounts.update(&uuid, |p| {
                let me = p.user_id.clone();
                let changed = p.progress.town.tick(&me, &mut p.progress.presents, now);
                let out = p.progress.town.town.as_ref().map(|t| login_notify(t, &Me::of(p), now));
                (out, if changed { Change::Progress } else { Change::None })
            });
            match r {
                Ok(Some((_, Some(notice)))) => {
                    s.uuid = Some(uuid);
                    replies.push((res, ok_response()));
                    replies.push((NOTICE_LOGIN, notice));
                }
                _ => {
                    replies.push((res, json!({ "result": 1, "status": E_CONNECT_CLAN })));
                    close = true;
                }
            }
        }
        REQ_HEART_BEAT => replies.push((
            res,
            json!({ "result": 0, "status": E_OK, "maintenance_flg": false, "maintenance_message": "" }),
        )),
        REQ_LOGOUT => {
            replies.push((res, ok_response()));
            close = true;
        }
        REQ_CHAR_MOVE => {
            s.walk(pos_of(msg));
            replies.push((res, ok_response()));
        }
        _ => {
            if pid == REQ_SET_CLAN_BG {
                // A new map: the walked spots belong to the old one.
                s.walked.clear();
                s.my_pos = None;
            }
            let Some(uuid) = s.uuid.clone() else {
                return Outcome {
                    replies: vec![(res, ok_response())],
                    close: false,
                };
            };
            replies.push((res, ok_response()));
            let r = ctx.accounts.update(&uuid, |p| {
                let me = Me::of(p);
                let Some(t) = p.progress.town.town.as_mut() else {
                    return (Vec::new(), Change::None);
                };
                let mut notes = Vec::new();
                let change = match pid {
                    REQ_CHAT_SEND => {
                        let text = msg.get("message").and_then(Value::as_str).unwrap_or("");
                        if text.trim().is_empty() {
                            Change::None
                        } else {
                            let line = t.add_chat(&me.id, text, now);
                            t.read_seq = line.seq;
                            notes.push((NOTICE_CHAT_SEND, chat_notify(&line)));
                            Change::Bookkeeping
                        }
                    }
                    REQ_CHAR_ACTION => Change::None,
                    REQ_CHAT_READED => {
                        let seq = msg.get("seq_id").and_then(Value::as_i64).unwrap_or(0);
                        if seq > t.read_seq {
                            t.read_seq = seq.min(t.seq);
                            Change::Bookkeeping
                        } else {
                            Change::None
                        }
                    }
                    REQ_SET_CLAN_OBJECT => {
                        if t.position_of(&me.id) >= town::POS_SUB
                            && town::functions(t.level()).contains(&"TOWN_FUNCTION_OBJECT_SET")
                        {
                            let mut placed: Vec<[i64; 2]> = Vec::new();
                            for o in msg.get("clan_object").and_then(Value::as_array).into_iter().flatten() {
                                let id = o.get("object_id").and_then(Value::as_i64).unwrap_or(0);
                                let slot = o.get("slot_index").and_then(Value::as_i64).unwrap_or(-1);
                                if id > 0 && slot >= 0 && t.objects.contains(&id)
                                    && !placed.iter().any(|p| p[1] == slot)
                                {
                                    placed.push([id, slot]);
                                }
                            }
                            t.placed = placed;
                        }
                        notes.push((NOTICE_SET_CLAN_OBJECT, set_object_notify(t)));
                        Change::Progress
                    }
                    REQ_SET_CLAN_BG => {
                        let bg = msg.get("background_id").and_then(Value::as_i64).unwrap_or(0);
                        if t.position_of(&me.id) >= town::POS_SUB
                            && town::backgrounds_for(t.level(), t.background).contains(&bg)
                        {
                            t.background = bg;
                            // A new map: everybody starts again near its origin (M).
                            t.my_pos = [0.0; 3];
                            let mut rng = crate::services::ranking::Rng(now as u64);
                            for m in &mut t.members {
                                m.pos = [
                                    (rng.below(201) as f64 - 100.0) / 100.0 * 1.5,
                                    0.0,
                                    (rng.below(201) as f64 - 100.0) / 100.0 * 1.5,
                                ];
                            }
                        }
                        notes.push((NOTICE_SET_CLAN_BG, set_bg_notify(t, &me)));
                        Change::Progress
                    }
                    _ => Change::None,
                };
                (notes, change)
            });
            if let Ok(Some((_, notes))) = r {
                replies.extend(notes);
            }
        }
    }
    Outcome { replies, close }
}

/// One AI member moves to a spot the player walked, or does its action (ours).
pub fn ai_step(ctx: &Ctx, s: &Session, tick: u64) -> Vec<(i32, Value)> {
    let Some(uuid) = &s.uuid else {
        return Vec::new();
    };
    let Ok(Some(p)) = ctx.accounts.get(uuid) else {
        return Vec::new();
    };
    let Some(t) = p
        .progress
        .town
        .town
        .as_ref()
        .filter(|t| !t.members.is_empty())
    else {
        return Vec::new();
    };
    let mut rng = crate::services::ranking::Rng(tick.wrapping_mul(0x9e37_79b9) ^ t.seq as u64);
    let m = &t.members[rng.below(t.members.len())];
    if rng.below(4) == 0 || s.walked.is_empty() {
        return vec![(NOTICE_CHAR_ACTION, action_notify(&m.id))];
    }
    let to = s.walked[rng.below(s.walked.len())];
    vec![(NOTICE_CHAR_MOVE, move_notify(&m.id, to))]
}

/// Serve one connection until it closes.
pub fn serve(ctx: Arc<Ctx>, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_nodelay(true);
    let mut buf = Vec::new();
    let mut s = Session::default();
    let mut header = Header::default();
    let mut last_ai = Instant::now();
    let mut tick = 0u64;
    let mut tmp = [0u8; 4096];
    log::line("town", "socket connected");
    'conn: loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
        loop {
            match take_frame(&mut buf) {
                Ok(Some((h, msg))) => {
                    header = h;
                    if ctx.trace {
                        log::line("town", &format!("<- {} {msg}", h.pid));
                    }
                    let out = handle(&ctx, &mut s, h.pid, &msg);
                    for (pid, m) in &out.replies {
                        if stream.write_all(&encode_frame(header, *pid, m)).is_err() {
                            break 'conn;
                        }
                    }
                    if out.close {
                        break 'conn;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::line("town", &format!("socket closed: {e}"));
                    break 'conn;
                }
            }
        }
        if last_ai.elapsed() >= AI_INTERVAL {
            last_ai = Instant::now();
            tick += 1;
            for (pid, m) in ai_step(&ctx, &s, tick) {
                if stream.write_all(&encode_frame(header, pid, &m)).is_err() {
                    break 'conn;
                }
            }
        }
    }
    s.save(&ctx);
    log::line("town", "socket closed");
}

/// The hook the hosts give the HTTP server.
pub fn hook(ctx: Arc<Ctx>) -> jdr_core::http::StreamHook {
    jdr_core::http::StreamHook {
        magic: MAGIC,
        handler: Arc::new(move |stream| serve(ctx.clone(), stream)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgpack_round_trips_in_the_old_spec() {
        let v = json!({ "a": 1, "b": -5, "c": 300, "d": 70000, "e": "x".repeat(40), "f": [1.5, true, null],
                        "g": -200, "h": 0.0 });
        let mut b = Vec::new();
        pack(&v, &mut b);
        // No str8 (0xd9): the client's reader predates it.
        assert!(!b.contains(&0xd9));
        let back = unpack(&b).unwrap();
        assert_eq!(back["a"], 1);
        assert_eq!(back["b"], -5);
        assert_eq!(back["c"], 300);
        assert_eq!(back["d"], 70000);
        assert_eq!(back["e"].as_str().unwrap().len(), 40);
        assert_eq!(back["f"][0], 1.5);
        assert_eq!(back["g"], -200);
        // A whole float stays a float32.
        let mut b = Vec::new();
        pack(&f(2.0), &mut b);
        assert_eq!(b[0], 0xca);
    }

    #[test]
    fn frames_round_trip() {
        let h = Header {
            version: 31200,
            platform: 2,
            size: 0,
            pid: 0,
            user_id: 100000001,
        };
        let mut buf = encode_frame(h, REQ_HEART_BEAT, &json!({}));
        assert_eq!(&buf[..4], b"GLPH");
        buf.extend_from_slice(&encode_frame(h, REQ_CHAT_SEND, &json!({ "message": "hi" })));
        let (h1, m1) = take_frame(&mut buf).unwrap().unwrap();
        assert_eq!(h1.pid, REQ_HEART_BEAT);
        assert_eq!(h1.user_id, 100000001);
        assert_eq!(m1, json!({}));
        let (h2, m2) = take_frame(&mut buf).unwrap().unwrap();
        assert_eq!(h2.pid, REQ_CHAT_SEND);
        assert_eq!(m2["message"], "hi");
        assert!(buf.is_empty());
        let mut half = encode_frame(h, REQ_LOGOUT, &json!({}));
        half.truncate(20);
        assert!(take_frame(&mut half).unwrap().is_none());
        let mut bad = b"POST / HTTP/1.1\r\n\r\n12345678".to_vec();
        assert!(take_frame(&mut bad).is_err());
    }

    fn read_frame(s: &mut TcpStream, buf: &mut Vec<u8>) -> (Header, Value) {
        let mut tmp = [0u8; 4096];
        loop {
            if let Some(f) = take_frame(buf).unwrap() {
                return f;
            }
            let n = s.read(&mut tmp).unwrap();
            assert!(n > 0, "closed");
            buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Read frames until one with `pid` (AI notices may come in between).
    fn expect(s: &mut TcpStream, buf: &mut Vec<u8>, pid: i32) -> Value {
        for _ in 0..20 {
            let (h, m) = read_frame(s, buf);
            if h.pid == pid {
                return m;
            }
        }
        panic!("no {pid}");
    }

    #[test]
    fn a_town_home_session_over_tcp() {
        use crate::services::shapes::assert_complete;
        let dir = std::env::temp_dir().join(format!("jdr-townsock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Arc::new(Ctx::new(jdr_core::settings::Settings::new(dir), false));
        let p = ctx.accounts.create(&[]).unwrap();
        let env = jdr_core::envelope::RequestEnvelope {
            uuid: p.uuid.clone(),
            session: String::new(),
            language: 1,
            date: 20261118103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param: json!({ "clanName": "Home", "policy": 1, "activityTime": 1, "approval": 1,
                           "emblem": "ce_0000.bytes", "comment": "" }),
        };
        let (r, _) = crate::services::dispatch(&ctx, "/clan/create", &env);
        r.unwrap();
        let (l, port) = jdr_core::http::bind(&[0]).unwrap();
        let h: jdr_core::http::Handler = Arc::new(|_r| jdr_core::http::Response::text(404, "no"));
        let hk = hook(ctx.clone());
        std::thread::spawn(move || jdr_core::http::serve_with(l, h, Some(hk)));
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let hd = Header {
            version: 31200,
            platform: 2,
            user_id: p.user_id.parse().unwrap(),
            ..Default::default()
        };
        let mut buf = Vec::new();
        s.write_all(&encode_frame(hd, REQ_LOGIN, &json!({ "uu_id": p.uuid })))
            .unwrap();
        let res = expect(&mut s, &mut buf, REQ_LOGIN + RES_OFFSET);
        assert_eq!(res["status"], E_OK);
        assert_eq!(res["result"], 0);
        assert_complete("Jjs.Socket.LoginResponse", &res);
        let notice = expect(&mut s, &mut buf, NOTICE_LOGIN);
        assert_complete("Jjs.Socket.LoginNotify", &notice);
        let members = notice["member"].as_array().unwrap();
        assert_eq!(members.len(), 4, "the player and three AI members");
        assert_eq!(members[0]["user_id"], p.user_id.as_str());
        assert!(members.iter().all(|m| m["x"].is_f64()));
        assert!(!members[1]["model_list"].as_array().unwrap().is_empty());
        // Heartbeat, move, chat, chat read.
        s.write_all(&encode_frame(hd, REQ_HEART_BEAT, &json!({})))
            .unwrap();
        let hb = expect(&mut s, &mut buf, REQ_HEART_BEAT + RES_OFFSET);
        assert_complete("Jjs.Socket.HeartBeatResponse", &hb);
        assert_eq!(hb["maintenance_flg"], false);
        s.write_all(&encode_frame(
            hd,
            REQ_CHAR_MOVE,
            &json!({ "x": 3.0, "y": 0.0, "z": 4.0 }),
        ))
        .unwrap();
        let mv = expect(&mut s, &mut buf, REQ_CHAR_MOVE + RES_OFFSET);
        assert_complete("Jjs.Socket.CharMoveResponse", &mv);
        s.write_all(&encode_frame(
            hd,
            REQ_CHAT_SEND,
            &json!({ "message": "hello" }),
        ))
        .unwrap();
        assert_complete(
            "Jjs.Socket.ChatResponse",
            &expect(&mut s, &mut buf, REQ_CHAT_SEND + RES_OFFSET),
        );
        let chat = expect(&mut s, &mut buf, NOTICE_CHAT_SEND);
        assert_complete("Jjs.Socket.ChatNotify", &chat);
        assert_eq!(chat["message"], "hello");
        assert_eq!(chat["seq_id"], 1);
        assert_eq!(chat["message_at"].as_str().unwrap().len(), 19);
        // Objects need Town Rank 8: below it the layout stays empty.
        s.write_all(&encode_frame(
            hd,
            REQ_SET_CLAN_OBJECT,
            &json!({ "user_id": p.user_id, "clan_object": [{ "object_id": 27, "slot_index": 0 }] }),
        ))
        .unwrap();
        let so = expect(&mut s, &mut buf, NOTICE_SET_CLAN_OBJECT);
        assert_complete("Jjs.Socket.SetClanObjectNotify", &so);
        assert_eq!(so["clan_object"], json!([]));
        ctx.accounts
            .update(&p.uuid, |q| {
                let t = q.progress.town.town.as_mut().unwrap();
                t.point = crate::town::level_total(10);
                t.objects = crate::town::objects_for(10);
                ((), Change::Progress)
            })
            .unwrap();
        s.write_all(&encode_frame(
            hd,
            REQ_SET_CLAN_OBJECT,
            &json!({ "user_id": p.user_id, "clan_object": [{ "object_id": 27, "slot_index": 0 }] }),
        ))
        .unwrap();
        assert_complete(
            "Jjs.Socket.SetClanObjectResponse",
            &expect(&mut s, &mut buf, REQ_SET_CLAN_OBJECT + RES_OFFSET),
        );
        let so = expect(&mut s, &mut buf, NOTICE_SET_CLAN_OBJECT);
        assert_eq!(
            so["clan_object"],
            json!([{ "object_id": 27, "slot_index": 0 }])
        );
        s.write_all(&encode_frame(
            hd,
            REQ_SET_CLAN_BG,
            &json!({ "user_id": p.user_id, "background_id": 9003 }),
        ))
        .unwrap();
        let bg = expect(&mut s, &mut buf, NOTICE_SET_CLAN_BG);
        assert_complete("Jjs.Socket.SetClanBGNotify", &bg);
        assert_eq!(bg["background_id"], 9003);
        s.write_all(&encode_frame(hd, REQ_LOGOUT, &json!({})))
            .unwrap();
        assert_complete(
            "Jjs.Socket.LogoutResponse",
            &expect(&mut s, &mut buf, REQ_LOGOUT + RES_OFFSET),
        );
        let q = ctx.accounts.get(&p.uuid).unwrap().unwrap();
        let t = q.progress.town.town.unwrap();
        assert_eq!(t.background, 9003);
        assert_eq!(t.chat.len(), 1);
        assert_eq!(t.read_seq, 1);
    }

    #[test]
    fn ai_members_walk_where_the_player_walked() {
        use crate::services::shapes::assert_complete;
        let dir = std::env::temp_dir().join(format!("jdr-townsock-ai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(jdr_core::settings::Settings::new(dir), false);
        let p = ctx.accounts.create(&[]).unwrap();
        let now = jdr_core::clock::now();
        ctx.accounts
            .update(&p.uuid, |q| {
                let t = crate::town::ai_town(crate::town::AI_TOWN_FIRST, now).unwrap();
                q.progress.town.join(t, &mut q.progress.presents, now);
                ((), Change::Progress)
            })
            .unwrap();
        let mut s = Session::default();
        let out = handle(&ctx, &mut s, REQ_LOGIN, &json!({ "uu_id": p.uuid }));
        assert!(!out.close);
        // Before the player moves, AI members only act.
        for tick in 0..8 {
            for (pid, m) in ai_step(&ctx, &s, tick) {
                assert_eq!(pid, NOTICE_CHAR_ACTION);
                assert_complete("Jjs.Socket.CharActionNotify", &m);
            }
        }
        handle(
            &ctx,
            &mut s,
            REQ_CHAR_MOVE,
            &json!({ "x": 2.5, "y": 0.0, "z": -1.0 }),
        );
        handle(
            &ctx,
            &mut s,
            REQ_CHAR_MOVE,
            &json!({ "x": 2.6, "y": 0.0, "z": -1.0 }),
        );
        assert_eq!(s.walked.len(), 1, "close spots are kept once");
        let moves: Vec<Value> = (0..40)
            .flat_map(|t| ai_step(&ctx, &s, t))
            .filter(|(pid, _)| *pid == NOTICE_CHAR_MOVE)
            .map(|(_, m)| m)
            .collect();
        assert!(!moves.is_empty());
        for m in &moves {
            assert_complete("Jjs.Socket.CharMoveNotify", m);
            assert_eq!(m["x"], 2.5);
            assert_eq!(m["z"], -1.0);
        }
        s.save(&ctx);
        let q = ctx.accounts.get(&p.uuid).unwrap().unwrap();
        assert_eq!(q.progress.town.town.unwrap().my_pos, [2.6, 0.0, -1.0]);
        // A bad position never reaches the save.
        handle(&ctx, &mut s, REQ_CHAR_MOVE, &json!({ "x": 1e30, "y": "a" }));
        assert_eq!(s.my_pos, Some([0.0; 3]));
    }

    #[test]
    fn login_without_a_town_is_refused() {
        let dir = std::env::temp_dir().join(format!("jdr-townsock-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(jdr_core::settings::Settings::new(dir), false);
        let p = ctx.accounts.create(&[]).unwrap();
        let mut s = Session::default();
        let out = handle(&ctx, &mut s, REQ_LOGIN, &json!({ "uu_id": p.uuid }));
        assert!(out.close);
        assert_eq!(out.replies[0].1["status"], E_CONNECT_CLAN);
        assert!(s.uuid.is_none());
    }
}
