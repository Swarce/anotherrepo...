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

/// The Notices screen's pages (services/menu.rs notify_list).
pub const NOTES_PAGE: &str = "/information/build_notes.html";
pub const MAINTENANCE_PAGE: &str = "/information/maintenance.html";
pub const ABOUT_PAGE: &str = "/information/about.html";

/// What this build adds, for the Notices screen's Updates tab (our words; no game text).
const NOTES: &[&str] = &[
    "Daily quests rotate by weekday; the day turns at 04:00 JST. The Friend Point summon has one free draw a day.",
    "Labyrinth: Beginner, Intermediate and Advanced dungeons. A battle or a Tarot pick left unfinished can be resumed from My Page.",
    "Story battles left unfinished can be resumed from My Page too.",
    "Arena: battles cost BP. A team's cost rises after each battle, every fourth battle lowers the others, and a full bar resets them. BP comes back over time; BP Recovery Colas and Diamond Coins restore it.",
    "26 assist-only statues join the collection. They can be equipped as assists, with their own assist ability and skill.",
    "Quests offer AI supporters each day. A supporter's leader ability helps the team in tactical battles, and a clear with one earns extra Friend Points.",
    "These notices.",
];

fn notes() -> Response {
    let items: String = NOTES.iter().map(|n| format!("<li>{n}</li>")).collect();
    html(
        &format!("Build {}", jdr_core::BUILD),
        &format!(
            "<p>What is new in this offline build.</p><ul>{items}</ul>\
             <p>Values the original servers held (prices, rewards, enemy strength) are this project's choices.</p>"
        ),
    )
}

pub fn page(rel: &str) -> Response {
    match rel {
        NOTES_PAGE => notes(),
        MAINTENANCE_PAGE => html(
            "Maintenance",
            "<p>This copy runs on a local server on your own device, so there is no maintenance and no downtime.</p>",
        ),
        ABOUT_PAGE => html(
            "About this build",
            "<p>An unofficial, non-commercial preservation build. The online service has ended; everything runs locally, with no purchases and no tracking.</p>",
        ),
        "/information/terms_a.html" => html(
            "Offline preservation build",
            "<p>The online service for this game has ended. This copy talks only to a local preservation server on your own devices. There are no purchases and no tracking.</p><p>Tap <b>Accept</b> to continue.</p>",
        ),
        "/information/copyright.html" => html(
            "Credits",
            "<p>Unofficial, non-commercial preservation build. All rights to the game and to JoJo's Bizarre Adventure belong to their respective owners.</p>",
        ),
        _ => html("Offline", "<p>This page was part of the online service and is not available offline.</p>"),
    }
}
