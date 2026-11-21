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

fn tr() -> Option<&'static crate::l10n::Translations> {
    static CELL: std::sync::OnceLock<Option<crate::l10n::Translations>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| crate::l10n::Translations::embedded().ok())
        .as_ref()
}

/// Our English for a label of a client text table, else the label.
fn text(table: &str, label: &str) -> String {
    tr().and_then(|t| t.english(&format!("text:{table}:{label}")))
        .unwrap_or(label)
        .replace('\n', " ")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The event page Event Home opens (`eventInfo.htmlLink`): the offline rules of the event, in our
/// words (docs/research/25-events-raids.md).
fn event_page(id: &str) -> Option<Response> {
    use crate::services::event::{self, Kind};
    let e = crate::quests::event_by_id(id)?;
    let name = text(
        "event_campaign_JP",
        &format!("EQ_{}_Name", event::label_id(id)),
    );
    let item = |n: i64| esc(&text("Item_Text_JP", &format!("ITEM_{n:04}_NAME")));
    let rules = match event::kind_of(e) {
        Kind::Exchange => format!(
            "<p>Every event quest you clear drops {} {}. Trade them for materials with the Exchange button.</p>",
            event::CURRENCY_PER_CLEAR,
            item(e.currency)
        ),
        Kind::Point => format!(
            "<p>Every event quest you clear earns {} event points. Rewards arrive in the Message Box as your points reach them; Check Rewards lists them. The ranking places you among AI rivals.</p>",
            event::POINTS_PER_CLEAR
        ),
        Kind::HighScore | Kind::HighScoreShop => {
            let shop = if event::kind_of(e) == Kind::HighScoreShop {
                format!(
                    " Every clear also drops {} (on a high-score stage one per {} points, 5 to 100), for the Exchange.",
                    item(e.currency),
                    event::SCORE_PER_COIN
                )
            } else {
                String::new()
            };
            format!(
                "<p>High-score stages score each win by their rule (shown before the battle): a fast clear scores up to {} (one turn), {} less per further turn; the largest single hit up to {}; the total damage up to {}. Each stage keeps its best score; your ranking total is the sum of your stage bests.{shop}</p>",
                event::TURN_MAX,
                event::TURN_STEP,
                event::MAX_DAMAGE_CAP,
                event::TOTAL_DAMAGE_CAP
            )
        }
        Kind::Quiz => format!(
            "<p>Each Quiz Quest asks a question in its Question Stage. Clear it, then clear the Answer Stage you think is right: the first answer you clear counts. A right answer gives {r} {i}, a wrong one {w}; other event quests give {o}. Certificates (Check Rewards) arrive in the Message Box as your {i} total grows.</p>",
            r = event::QUIZ_RIGHT,
            w = event::QUIZ_WRONG,
            o = event::QUIZ_PER_CLEAR,
            i = item(e.currency)
        ),
    };
    let statues: Vec<String> = e
        .bonus_statues
        .iter()
        .map(|s| esc(&text("StatueName_JP", &format!("STATUE_NAME_{s}"))))
        .collect();
    let minds: Vec<String> = e
        .bonus_minds
        .iter()
        .map(|m| esc(&text("MindName_JP", &format!("MIND_NAME_{m}"))))
        .collect();
    let bonus = format!(
        "<h2>Event bonus</h2><p>Each of these statues in your team adds {}% to what a clear drops (points, coins, items; not the score): {}.</p><p>Each of these Minds on the team adds {}%: {}. At most +{}%.</p>",
        event::BONUS_STATUE_PCT,
        statues.join(", "),
        event::BONUS_MIND_PCT,
        minds.join(", "),
        event::BONUS_CAP_PCT
    );
    Some(html(
        &esc(&name),
        &format!("{rules}{bonus}<p>Offline, events never close.</p>"),
    ))
}

fn campaign_page(id: &str) -> Option<Response> {
    let c = crate::quests::campaigns()
        .iter()
        .find(|c| c.campaign_id == id)?;
    Some(html(
        &esc(&text("event_campaign_JP", &c.name)),
        "<p>This campaign belonged to the online service. Offline its banner is kept for the record; nothing changes in play.</p>",
    ))
}

/// The Notices screen's pages (services/menu.rs notify_list).
pub const NOTES_PAGE: &str = "/information/build_notes.html";
pub const MAINTENANCE_PAGE: &str = "/information/maintenance.html";
pub const ABOUT_PAGE: &str = "/information/about.html";

/// What this build adds, for the Notices screen's Updates tab (our words; no game text).
const NOTES: &[&str] = &[
    "Build 34: the same game data as the last Build 33 package, numbered 34 so you can see at a glance that the new app is running. Battle cameras follow the kind of action (attacks stay near the turn view, SP skills cut between several angles), the win shot is the original's close low shot, and the My Page and team screens frame statues as the original did.",
    "Build 33: the battle camera cuts to its own shots on every attack and skill, as the original did, and bosses get their intro again. Action chapters: enemies appear and your statues can attack, use their skill and SP. Skill cool turns follow the original's; more statues have their real stats; assist abilities need their real conditions.",
    "Build 32: long statue names shrink to fit their plate (two lines at most) instead of spilling onto the tabs, and the Town list's policy and hours fit their bars. The analytics, crash-reporting, ads, push and Play Games libraries are gone from the app (about 8 MB), with the Play Games buttons.",
    "Build 31: the generated portraits show in the statue lists (the game could not find them).",
    "Build 28: the generated portraits keep a real portrait's frame, stars and badge pixel for pixel; only the inside is new.",
    "Build 27: statues the game files have no portrait for get one, drawn from their menu pose with the frame, stars and attribute of the others.",
    "Build 26: the Town home connects without a network (it showed a connection error offline).",
    "Daily quests rotate by weekday; the day turns at 04:00 JST. The Friend Point summon has one free draw a day.",
    "Labyrinth: Beginner, Intermediate and Advanced dungeons. A battle or a Tarot pick left unfinished can be resumed from My Page.",
    "Story battles left unfinished can be resumed from My Page too.",
    "Arena: battles cost BP. A team's cost rises after each battle, every fourth battle lowers the others, and a full bar resets them. BP comes back over time; BP Recovery Colas and Diamond Coins restore it.",
    "26 assist-only statues join the collection. They can be equipped as assists, with their own assist ability and skill.",
    "Quests offer AI supporters each day. A supporter's leader ability helps the team in tactical battles, and a clear with one earns extra Friend Points.",
    "The Town: create and lead your own Town or join one of twelve AI Towns. Members walk the Town home, donations of Town Medals raise its level and unlock objects, and rewards arrive in the Message Box.",
    "New events: high-score events with their own scoring rules and a ranking, a quiz event and two more exchange events. Each event has bonus statues and Minds that raise its drops.",
    "Skill Minds add a second skill or SP skill in the two skill frames. Growth tree skill squares raise skill levels, which shorten cool turns.",
    "Mind Links: Minds of the same link on one statue raise its ATK, DEF or HP.",
    "DIO's and Jotaro's time-stop skills stop time.",
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
    let tail = |prefix: &str| {
        rel.strip_prefix(prefix)
            .and_then(|r| r.strip_suffix(".html"))
            .map(str::to_owned)
    };
    if let Some(r) = tail("/information/event_").and_then(|id| event_page(&id)) {
        return r;
    }
    if let Some(r) = tail("/information/campaign_").and_then(|id| campaign_page(&id)) {
        return r;
    }
    match rel {
        "/information/arena.html" => html(
            "Arena",
            "<p>Offline, the Arena season runs every week against AI rivals built from the roster. Durability, Fever and the season rewards follow the 2019 rules.</p>",
        ),
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
