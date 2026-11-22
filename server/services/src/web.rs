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

/// The Notices posts (`server/content/news.json`, our words; docs/CONTENT.md "Notices").
pub const NEWS_JSON: &str = include_str!("../../content/news.json");
/// The notice pages' style: our own CSS, every shape drawn in CSS (no retail images or sheet).
const NOTICE_CSS: &str = include_str!("../../content/notice.css");
/// Collapses every post but the first. Without it (scripts off) every post stays open: the page
/// is sent expanded and only this script folds it (docs/research/27-support-and-news.md §2.2).
/// A tap toggles a post. First the script sets `--edge` (notice.css): how
/// much wider the view is than the box the game designed for it, 1200x720 of its 1280x720 layout
/// (Notices offsets 40 left and right), so 5/3 of the screen height on a wide screen. Only on a
/// phone whose cutout widens the web view (docs/research/27-support-and-news.md §2.3) is it more
/// than 0; at most a fifth of the view.
const NOTICE_JS: &str = "(function(){function fit(){var s=window.screen,h=Math.min(s.width,s.height),w=window.innerWidth,b=h*5/3,e=(w-b)/2+b*0.015;e=e>1?Math.min(e,w*0.2):0;document.documentElement.style.setProperty('--edge',Math.floor(e)+'px');}fit();window.addEventListener('resize',fit);})();document.querySelectorAll('.headline').forEach(function(b,i){var s=document.getElementById(b.getAttribute('aria-controls'));function set(o){s.hidden=!o;b.setAttribute('aria-expanded',o?'true':'false');}set(i===0);b.addEventListener('click',function(){set(s.hidden);});});";

#[derive(serde::Deserialize)]
struct News {
    posts: Vec<Post>,
}

#[derive(serde::Deserialize)]
pub struct Post {
    pub id: String,
    /// The build that brings the post; it is shown from that build on.
    #[serde(default)]
    pub build: Option<u32>,
    #[serde(default)]
    pub date: String,
    pub title: String,
    /// `update` | `fault` | `other`: the tab it is on.
    pub category: String,
    pub blocks: Vec<Block>,
}

#[derive(serde::Deserialize)]
pub struct Block {
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

fn all_posts() -> &'static [Post] {
    static CELL: std::sync::OnceLock<Vec<Post>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        serde_json::from_str::<News>(NEWS_JSON)
            .expect("server/content/news.json")
            .posts
    })
}

/// The posts of one tab this build shows, in file order (newest first).
pub fn posts(category: &str) -> impl Iterator<Item = &'static Post> + '_ {
    all_posts().iter().filter(move |p| {
        p.category == category && !matches!(p.build, Some(b) if b > jdr_core::BUILD)
    })
}

fn esc_attr(s: &str) -> String {
    esc(s).replace('"', "&quot;").replace('\'', "&#39;")
}

/// Rich text → HTML: `<color=#RRGGBB>`, `<b>`, `<i>` (properly nested) and `\n` are kept, anything
/// else is shown as text; tags left open are closed at the end of the block.
pub fn rich(t: &str) -> String {
    let mut out = String::new();
    let mut open: Vec<&str> = Vec::new();
    let mut rest = t;
    while let Some(i) = rest.find('<') {
        out.push_str(&esc(&rest[..i]));
        let tail = &rest[i..];
        let tag = |name: &str| tail.starts_with(name).then_some(name.len());
        let color = tail
            .strip_prefix("<color=#")
            .filter(|c| c.len() > 6 && c.as_bytes()[6] == b'>')
            .map(|c| &c[..6])
            .filter(|h| h.bytes().all(|x| x.is_ascii_hexdigit()));
        let used = if let Some(hex) = color {
            out.push_str(&format!("<span style=\"color:#{hex}\">"));
            open.push("color");
            Some(15)
        } else if let Some(n) = tag("<b>").or_else(|| tag("<i>")) {
            let k = &tail[1..2];
            out.push_str(&tail[..n]);
            open.push(if k == "b" { "b" } else { "i" });
            Some(n)
        } else {
            let close = [("</color>", "color"), ("</b>", "b"), ("</i>", "i")]
                .into_iter()
                .find(|(c, k)| tail.starts_with(c) && open.last() == Some(k));
            close.map(|(c, k)| {
                open.pop();
                out.push_str(if k == "color" { "</span>" } else { c });
                c.len()
            })
        };
        let n = used.unwrap_or_else(|| {
            out.push_str("&lt;");
            1
        });
        rest = &tail[n..];
    }
    out.push_str(&esc(rest));
    while let Some(k) = open.pop() {
        out.push_str(match k {
            "color" => "</span>",
            "b" => "</b>",
            _ => "</i>",
        });
    }
    out.replace('\n', "<br>")
}

/// One post: a headline button (date above title) and its body; unknown block kinds are skipped.
fn render_post(p: &Post) -> String {
    let id: String =
        p.id.to_ascii_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
    let body: String = p
        .blocks
        .iter()
        .filter_map(|b| match b.kind.as_str() {
            "heading" => Some(format!("<p class=\"subhead\">{}</p>", rich(&b.text))),
            "paragraph" => Some(format!("<p class=\"paragraph\">{}</p>", rich(&b.text))),
            "note" => Some(format!("<p class=\"note\">{}</p>", rich(&b.text))),
            // Our own images only, next to the page (`<name>.png`); a name is [a-z0-9_-].
            "banner"
                if !b.text.is_empty()
                    && b.text.bytes().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
                    }) =>
            {
                Some(format!(
                    "<img class=\"banner\" src=\"{}.png\" alt=\"\">",
                    b.text
                ))
            }
            _ => None,
        })
        .collect();
    let date = if p.date.is_empty() {
        String::new()
    } else {
        format!(
            "<time class=\"date\" datetime=\"{d}\">{}</time>",
            esc(&p.date),
            d = esc_attr(&p.date)
        )
    };
    format!(
        "<article class=\"notice\" id=\"{id}\"><button type=\"button\" class=\"headline\" aria-expanded=\"true\" aria-controls=\"b-{id}\">{date}<span class=\"title\">{}</span></button><section class=\"content\" id=\"b-{id}\">{body}</section></article>",
        esc(&p.title)
    )
}

/// The notice pages' viewport: the layout is as wide as the web view, at scale 1, never zoomed.
/// The client's web view keeps Android's defaults (no wide viewport, no overview mode, no text
/// zoom, zoom off), which already lay a page out at the view's width; the meta says the same, so
/// a web view that honours it (wide viewport on) shows the page the same way.
/// docs/research/27-support-and-news.md §2.3.
const NOTICE_VIEWPORT: &str =
    "width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1, user-scalable=no";

/// A Notices tab: its posts as collapsible notices, CSS and script inline (no other files).
fn notice_page(title: &str, category: &str) -> Response {
    let posts: String = posts(category).map(render_post).collect();
    let s = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"{NOTICE_VIEWPORT}\"><title>{title}</title><style>{NOTICE_CSS}</style></head><body><main>{posts}</main><script>{NOTICE_JS}</script></body></html>"
    );
    Response::ok("text/html; charset=utf-8", s.into_bytes())
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
        NOTES_PAGE => notice_page("Updates", "update"),
        MAINTENANCE_PAGE => notice_page("Maintenance", "fault"),
        ABOUT_PAGE => notice_page("About this build", "other"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn body(rel: &str) -> String {
        String::from_utf8(page(rel).body).unwrap()
    }

    /// Only the kept rich-text tags become HTML; a `<script>` in a post stays text, a stray or
    /// badly nested tag is shown as text and an open tag is closed within its block.
    #[test]
    fn rich_text_is_escaped() {
        assert_eq!(
            rich("<script>alert('x')</script> & co"),
            "&lt;script&gt;alert('x')&lt;/script&gt; &amp; co"
        );
        assert_eq!(
            rich("<color=#FFD700><b>Gold</b></color>\nline <i>two</i>"),
            "<span style=\"color:#FFD700\"><b>Gold</b></span><br>line <i>two</i>"
        );
        assert_eq!(rich("</b>a<b>open"), "&lt;/b&gt;a<b>open</b>");
        assert_eq!(rich("<b><i>x</b></i>"), "<b><i>x&lt;/b&gt;</i></b>");
        assert_eq!(
            rich("<color=#12345>x<color=#12345z>"),
            "&lt;color=#12345&gt;x&lt;color=#12345z&gt;"
        );
        assert_eq!(rich("<color=red>x"), "&lt;color=red&gt;x");
        assert_eq!(rich("a <"), "a &lt;");
    }

    /// news.json is well formed: unique ids, known tabs, every tab has a post, Update posts carry
    /// their build and date, newest first; every post this build shows renders on its tab.
    #[test]
    fn every_post_renders() {
        let mut ids = std::collections::HashSet::new();
        for p in all_posts() {
            assert!(ids.insert(p.id.as_str()), "duplicate id {}", p.id);
            assert!(
                ["update", "fault", "other"].contains(&p.category.as_str()),
                "{}",
                p.id
            );
            assert!(!p.blocks.is_empty(), "{}", p.id);
        }
        let mut last = u32::MAX;
        for p in all_posts().iter().filter(|p| p.category == "update") {
            let b = p.build.unwrap_or_else(|| panic!("{} has no build", p.id));
            assert!(b < last, "{}: update posts go newest first", p.id);
            assert_eq!(p.id, format!("build-{b:03}"));
            assert_eq!(p.date.len(), 10, "{}: date YYYY-MM-DD", p.id);
            last = b;
        }
        for (rel, cat) in [
            (NOTES_PAGE, "update"),
            (MAINTENANCE_PAGE, "fault"),
            (ABOUT_PAGE, "other"),
        ] {
            let page = body(rel);
            let shown: Vec<&Post> = posts(cat).collect();
            assert!(!shown.is_empty(), "{cat}");
            assert_eq!(
                page.matches("<article class=\"notice\"").count(),
                shown.len()
            );
            for p in shown {
                assert!(page.contains(&format!("<span class=\"title\">{}</span>", esc(&p.title))));
                assert!(page.contains(&format!("id=\"b-{}\"", p.id)), "{}", p.id);
            }
        }
    }

    /// A block kind the renderer does not know is left out; a banner names one of our images.
    #[test]
    fn unknown_block_kinds_are_skipped() {
        let block = |kind: &str, text: &str| Block {
            kind: kind.into(),
            text: text.into(),
        };
        let p = Post {
            id: "Test Post".into(),
            build: None,
            date: String::new(),
            title: "T <1>".into(),
            category: "other".into(),
            blocks: vec![
                block("heading", "Head"),
                block("video", "clip"),
                block("paragraph", "Body"),
                block("banner", "../x"),
                block("banner", "event_01"),
                block("note", "Small"),
            ],
        };
        let h = render_post(&p);
        assert!(!h.contains("clip") && !h.contains("../x"));
        assert!(h.contains("id=\"test-post\"") && h.contains("T &lt;1&gt;"));
        assert!(h.contains(
            "<section class=\"content\" id=\"b-test-post\"><p class=\"subhead\">Head</p><p class=\"paragraph\">Body</p><img class=\"banner\" src=\"event_01.png\" alt=\"\"><p class=\"note\">Small</p></section>"
        ));
        assert!(!h.contains("<time"), "no date, no time element");
    }

    /// The pages are self-contained: CSS and script inline, nothing fetched from anywhere, and
    /// sent expanded (the inline script folds all but the first post).
    #[test]
    fn notice_pages_have_no_external_urls() {
        for rel in [NOTES_PAGE, MAINTENANCE_PAGE, ABOUT_PAGE] {
            let page = body(rel);
            for bad in [
                "://", "src=\"/", "href=", "url(", "@import", "<link", " hidden",
            ] {
                assert!(!page.contains(bad), "{rel}: {bad}");
            }
            assert!(
                page.contains("<style>") && page.contains("<script>"),
                "{rel}"
            );
            assert!(!page.contains("[["), "{rel}: a placeholder is shown");
        }
    }

    /// The pages fit any web view: the viewport is the view's width at scale 1, the type scales
    /// with the view within readable bounds, sizes are relative (no fixed pixel boxes), nothing
    /// is set wider than the view, sideways scrolling is off and long words wrap.
    #[test]
    fn notice_page_insets_by_half_the_extra_width() {
        // The web view is wider than the box (docs/research/27 §2.3): the page is inset on each
        // side by HALF of the extra width (plus a small margin), and asks for a light scheme so
        // the web view does not paint its dark default (#121212) around the page.
        assert!(NOTICE_JS.contains("e=(w-b)/2"));
        assert!(NOTICE_CSS.contains("color-scheme: only light"));
        assert!(!NOTICE_CSS.contains("color-scheme: dark"));
    }

    #[test]
    fn notice_pages_fit_the_view() {
        for rel in [NOTES_PAGE, MAINTENANCE_PAGE, ABOUT_PAGE] {
            let page = body(rel);
            let meta = format!("<meta name=\"viewport\" content=\"{NOTICE_VIEWPORT}\">");
            assert!(page.contains(&meta), "{rel}");
        }
        for part in ["width=device-width", "initial-scale=1", "maximum-scale=1"] {
            assert!(NOTICE_VIEWPORT.contains(part), "{part}");
        }
        let css: String = NOTICE_CSS
            .split("*/")
            .map(|s| s.split("/*").next().unwrap_or(""))
            .collect();
        for rule in [
            "font-size: clamp(13px, min(2.5vw, 6.4vh), 22px)",
            "html, body { width: 100%; max-width: 100%; overflow-x:hidden; }",
            "overflow-wrap: anywhere",
            "text-size-adjust: 100%",
            "img, table, pre { max-width: 100%; }",
            "html { background: transparent;",
            "margin: 0 var(--edge, 0px)",
        ] {
            assert!(css.contains(rule), "missing: {rule}");
        }
        // Only `main` is painted, inset by --edge, which the script sets from the view and screen.
        assert!(NOTICE_JS.contains("setProperty('--edge'") && NOTICE_JS.contains("h*5/3"));
        // Fixed sizes only for hairlines (borders, shadows), the root clamp and the breakpoint;
        // every other length is in em, rem, % or view units.
        let fixed: Vec<&str> = css
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
            .filter(|w| {
                w.strip_suffix("px").is_some_and(|n| {
                    n.parse::<f32>().is_ok_and(|v| v > 2.0) && !["13", "22", "480"].contains(&n)
                })
            })
            .collect();
        assert!(fixed.is_empty(), "fixed sizes: {fixed:?}");
        for bad in ["100vw", "min-width", "nowrap", "position: fixed"] {
            assert!(!css.contains(bad), "{bad}");
        }
    }

    /// The Update tab opens on this build's post; a post for a later build stays hidden.
    #[test]
    fn update_page_leads_with_the_current_build() {
        let page = body(NOTES_PAGE);
        let first = page.find("<article").unwrap();
        assert_eq!(
            &page[first..first + 38],
            format!(
                "<article class=\"notice\" id=\"build-{:03}\"",
                jdr_core::BUILD
            )
        );
        assert!(page.contains(&format!(">Build {}</span>", jdr_core::BUILD)));
        for p in all_posts() {
            if p.build.is_some_and(|b| b > jdr_core::BUILD) {
                assert!(!page.contains(&format!("id=\"{}\"", p.id)), "{}", p.id);
            }
        }
    }
}
