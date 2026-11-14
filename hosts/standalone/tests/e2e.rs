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
    call_as(port, path, param, "")
}

fn call_as(port: u16, path: &str, param: &str, uuid: &str) -> serde_json::Value {
    let js = format!(
        r#"{{"uuid":"{uuid}","session":"","language":1,"date":20261114103000,"platform":2,"version":"vtest","lzAccept":1,"lzEncode":0,"param":{param}}}"#
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
    assert_eq!(v["param"]["s3_url"], format!("http://127.0.0.1:{port}/web"));
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

fn get(port: u16, path: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n").as_bytes())
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

/// The new-player boot sequence in the client's order (docs/protocol/boot.md).
#[test]
fn new_player_boot_sequence() {
    let port = start(None);
    let v = call(port, "/common/get_version", "{}");
    let api = v["param"]["url"]
        .as_str()
        .unwrap()
        .trim_start_matches(&format!("http://127.0.0.1:{port}"))
        .to_string();
    let web = v["param"]["s3_url"]
        .as_str()
        .unwrap()
        .trim_start_matches(&format!("http://127.0.0.1:{port}"))
        .to_string();
    let ep = |p: &str| format!("{api}{p}");

    let login = call(
        port,
        &ep("/user/login"),
        r#"{"model":"SM-S901B","os":"Android OS 16","osVersion":"16","backupId":"x","ad_id":""}"#,
    );
    assert_eq!(
        login["param"]["existStatus"], 4,
        "unknown device -> new user"
    );
    assert_eq!(login["param"]["is_ca"], 0);
    assert!(get(port, &format!("{web}/information/terms_a.html")).starts_with("HTTP/1.1 200"));
    let nonce = call(port, &ep("/user/get_nonce"), "{}");
    assert!(!nonce["param"]["nonce"].as_str().unwrap().is_empty());
    let cu = call(
        port,
        &ep("/user/create_user"),
        r#"{"nonce":"n","signature":"s","backupId":"x","platform_user_id":"a","ad_id":""}"#,
    );
    let uuid = cu["param"]["uuId"].as_str().unwrap().to_string();
    assert!(!uuid.is_empty() && !cu["param"]["user_id"].as_str().unwrap().is_empty());

    let login2 = call_as(port, &ep("/user/login"), "{}", &uuid);
    assert_eq!(login2["param"]["existStatus"], 0);
    assert_eq!(login2["param"]["userInfoMini"]["username"], "Player");
    // My Page: the tactical main team with a leader statue (background + model)
    let team = &login2["param"]["teamInfo"];
    assert_eq!(
        (team["stageType"].as_i64(), team["mainFlg"].as_i64()),
        (Some(1), Some(1))
    );
    let leader = &team["teamOrder"]["leaderStatue"]["statueInfo"];
    assert!(leader["statueId"].as_str().unwrap().len() == 7, "{leader}");
    assert!(!leader["modelDataList"].as_array().unwrap().is_empty());
    let ti = call_as(port, &ep("/team/team_info"), "{}", &uuid);
    assert_eq!(
        ti["param"]["teamList"].as_array().unwrap().len(),
        10,
        "5 teams x 2 battle types"
    );
    let sl = call_as(port, &ep("/statue/statue_list"), "{}", &uuid);
    let n = sl["param"]["statueList"].as_array().unwrap().len();
    assert!(n > 200 && sl["param"]["allStatueNum"] == n, "{n}");
    let uid = leader["statueuId"].as_i64().unwrap();
    let si = call_as(
        port,
        &ep("/statue/statue_info"),
        &format!(r#"{{"statueuId":{uid}}}"#),
        &uuid,
    );
    assert_eq!(si["param"]["statueInfo"]["statueId"], leader["statueId"]);
    let ml = call_as(port, &ep("/mind/mind_list"), "{}", &uuid);
    assert!(ml["param"]["mindList"].as_array().unwrap().len() > 300);
    // Story: the story button, a chapter list, a stage list, start and end (clear recorded)
    let sel = call_as(port, &ep("/bizarre/get_bizarre_select"), "{}", &uuid);
    assert_eq!(sel["param"]["questList"][0]["bannerType"], 1);
    let bl = call_as(
        port,
        &ep("/bizarre/bizarre_list"),
        r#"{"category":1}"#,
        &uuid,
    );
    let ch = &bl["param"]["chapterList"][1];
    let chapter = ch["chapterId"].as_str().unwrap().to_string();
    let stl = call_as(
        port,
        &ep("/bizarre/stage_list"),
        &format!(
            r#"{{"chapterId":"{chapter}","stageType":{}}}"#,
            ch["stageType"]
        ),
        &uuid,
    );
    let stage = stl["param"]["stageList"][0]["stageId"]
        .as_str()
        .unwrap()
        .to_string();
    let st = call_as(
        port,
        &ep("/bizarre/bizarre_start"),
        &format!(r#"{{"stageId":"{stage}","teamNo":1}}"#),
        &uuid,
    );
    assert!(!st["param"]["areaList"].as_array().unwrap().is_empty());
    let en = call_as(
        port,
        &ep("/bizarre/bizarre_end"),
        &format!(r#"{{"stageId":"{stage}","result":0}}"#),
        &uuid,
    );
    assert_eq!(en["param"]["clearRank"], 4);
    assert!(en["param"]["gold"].as_i64().unwrap() > 0);
    // A resent end for the same battle pays nothing more.
    let again = call_as(
        port,
        &ep("/bizarre/bizarre_end"),
        &format!(r#"{{"stageId":"{stage}","result":0}}"#),
        &uuid,
    );
    assert_eq!(again["param"]["gold"], 0);
    assert_eq!(
        again["param"]["userInfo"]["vm"],
        en["param"]["userInfo"]["vm"]
    );
    let stl2 = call_as(
        port,
        &ep("/bizarre/stage_list"),
        &format!(
            r#"{{"chapterId":"{chapter}","stageType":{}}}"#,
            ch["stageType"]
        ),
        &uuid,
    );
    assert_eq!(stl2["param"]["stageList"][0]["clearFlg"], 1);

    let k = call_as(port, &ep("/client/getkey"), "{}", &uuid);
    assert_eq!(k["param"]["assetkey"], "A".repeat(16));
    let cfg = call_as(port, &ep("/common/get_system_config"), "{}", &uuid);
    assert!(
        cfg["param"]["configList"]["apOneRecoveryTime"]
            .as_i64()
            .unwrap()
            > 0
    );
    let tut = call_as(port, &ep("/user/get_tutorial_step"), "{}", &uuid);
    assert!(tut["param"]["tutorialStep"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x == 1));
    let fd = call_as(
        port,
        &ep("/client/getfiledata"),
        r#"{"filename":["android/data/aa.bytes","android/data/bb.bytes"]}"#,
        &uuid,
    );
    assert_eq!(
        fd["param"]["url"],
        serde_json::json!([
            format!("http://127.0.0.1:{port}/cdn/aa.bytes"),
            format!("http://127.0.0.1:{port}/cdn/bb.bytes")
        ])
    );
    assert_eq!(
        call_as(port, &ep("/shop/product_list"), "{}", &uuid)["param"]["key"],
        ""
    );

    for p in [
        "/user/user_info",
        "/team/team_info",
        "/info/check_forced_notify",
        "/info/event_list",
        "/info/banner_list",
        "/user/get_loginbonus",
        "/info/get_menu_notify",
    ] {
        let r = call_as(port, &ep(p), r#"{"id":""}"#, &uuid);
        assert_eq!(r["param"]["status"], 10000, "{p}");
    }
    assert_eq!(
        call_as(port, &ep("/bizarre/bizarre_resumption"), "{}", &uuid)["param"]["status"],
        50301
    );
    // MyPage menu screens: handled (not the unknown-route fallback), every list present and empty
    for (p, param, lists) in [
        (
            "/bizarre/get_bizarre_select",
            "{}",
            &["eventCampaignList"][..],
        ),
        (
            "/present/present_list",
            r#"{"prevNum":1,"receivedFlag":0}"#,
            &["presentList"][..],
        ),
        ("/follow/follow_list", "{}", &["followList"][..]),
        ("/item/item_list", "{}", &["itemList"][..]),
        ("/shop/shop_list", "{}", &["itemList"][..]),
    ] {
        let r = call_as(port, &ep(p), param, &uuid);
        assert_eq!(r["param"]["status"], 10000, "{p}");
        for f in lists {
            assert_eq!(r["param"][*f], serde_json::json!([]), "{p} {f}");
        }
    }
    // The main shop reads shopList[0].goodsList without a length check: never empty.
    let shop = call_as(port, &ep("/shop/shop_list"), "{}", &uuid);
    assert!(shop["param"]["shopList"][0]["goodsList"].is_array());
    // an unknown device asking for its profile gets a session error, not a crash
    assert_eq!(
        call_as(port, &ep("/user/user_info"), "{}", "nobody")["param"]["status"],
        10003
    );
}
