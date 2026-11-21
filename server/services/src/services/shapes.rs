//! Test-only: the client's response classes as field kinds, to walk a response the way the
//! client's JSON reader fills its objects. A string, list or object field the server leaves out
//! reads as null on the client, and one null string the Arena home dereferenced hung it on a
//! device (Build 23, `ItemInfo.icon`), so a walk reports every such field that is missing or
//! null, all the way down.
//!
//! `testdata/client_shapes.json` lists, per client class of 3.12.0 (dump.cs, inherited fields
//! included), its string ("s"), list ("l", with the element class when it is one) and object
//! ("o") fields. Numbers are left out: a missing number reads as 0.

use serde_json::Value;
use std::collections::BTreeMap;

const SHAPES: &str = include_str!("testdata/client_shapes.json");

type Shapes = BTreeMap<String, Vec<(String, String, Option<String>)>>;

fn shapes() -> &'static Shapes {
    static CELL: std::sync::OnceLock<Shapes> = std::sync::OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(SHAPES).expect("client_shapes.json"))
}

/// Fields the walk does not require, with the reason (client facts, H).
const EXEMPT: &[(&str, &str, &str)] = &[
    // A client-side countdown cache: GetRecoveryTimeSpan builds it from recoveryTime when null.
    ("Jjs.PvpTeamInfo", "recoveryCalcInfo", "client cache"),
    // UserInfo's three countdown caches, built by the client from the *Time strings.
    ("Jjs.UserInfo", "staminaLocalInfo", "client cache"),
    ("Jjs.UserInfo", "bpLocalInfo", "client cache"),
    ("Jjs.UserInfo", "rpLocalInfo", "client cache"),
    // No support player offline: the result screen tests UserInfoMini.Valid (a non-empty `id`)
    // and skips the support panel for `{}` (BattleResultUIController.SetUI, docs/research/25).
    (
        "Jjs.BizarreEndResponse",
        "supportUserInfo",
        "no support user",
    ),
    // A summon result fills the one of its three objects its dataType names (1 statue, 2 Mind);
    // the readers dereference only that one (GetStatueThumnailAssetName 0xE50AC4 reads
    // statueInfo of a statue result). Summons have run on a device with the other two null since
    // Build 10, so a `{}` there is not risked.
    ("Jjs.GashaDataInfo", "statueInfo", "by dataType"),
    ("Jjs.GashaDataInfo", "mindInfo", "by dataType"),
    ("Jjs.GashaDataInfo", "itemInfo", "by dataType"),
];

/// Sub-objects of the shared statue and Mind builders (collection::statue_info, mind_info) that
/// mean "none" when sent as `{}`: no leader ability, no assist skill, no status-effect rates, no
/// bonus-target (tokkou) ability. Their fields are private or protected in the client (filled
/// from the master tables, `isInit` false); the story battle, the team screens and the Arena
/// banners have read them this way since the tactical battle first ran.
///
/// Two battle objects mean "none" as `{}` too, as every story battle has sent them since the
/// first ones ran on a device (Build 06 onwards): `AreaInfo.mapGimmickInfo` (no map gimmick) and
/// `supportUserInfo` (a UserInfoMini: no friend support); and a story start's `tarotInfo` (no
/// card; the battle takes its Tarot from `selectTarotList`, docs/research/22 §1.8), sent so since
/// Build 23 with the story battles still playing on the device. A filled one is still walked in
/// full.
const NONE_AS_EMPTY: &[&str] = &[
    "Jjs.MapGimmickInfo",
    "Jjs.UserInfoMini",
    "Jjs.TarotInfo",
    "Jjs.AbilityInfo",
    "Jjs.DtAbilityInfo",
    "Jjs.SkillInfo",
    "Jjs.TokkouInfo",
    "Jjs.DtTokkouInfo",
    "Jjs.dtbattle.StEffRateInfo",
];

/// Every string, list or object field of `class` (and below) that `v` leaves out or sends as
/// null, as paths.
pub fn missing(class: &str, v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(class, v, class, &mut out);
    out
}

fn walk(class: &str, v: &Value, path: &str, out: &mut Vec<String>) {
    let Some(fields) = shapes().get(class) else {
        panic!("{class} is not in client_shapes.json");
    };
    let Value::Object(m) = v else {
        out.push(format!("{path}: not an object"));
        return;
    };
    if m.is_empty() && NONE_AS_EMPTY.contains(&class) {
        return;
    }
    // An empty seat (statueuId 0): the team banners and the formation reset the seat's icon
    // without reading its statueInfo (PvP2HomeTeamBannerController.UpdateUI, client), which
    // carries only `statueId: ""`.
    let empty_seat = class == "Jjs.StatueSetting" && m.get("statueuId") == Some(&Value::from(0));
    for (name, kind, elem) in fields {
        if empty_seat && name == "statueInfo" {
            if !m.get(name).is_some_and(Value::is_object) {
                out.push(format!("{path}.{name}: object expected"));
            }
            continue;
        }
        if EXEMPT.iter().any(|(c, f, _)| *c == class && f == name) {
            continue;
        }
        let p = format!("{path}.{name}");
        match (kind.as_str(), m.get(name)) {
            ("s", Some(Value::String(_))) => {}
            ("l", Some(Value::Array(a))) => {
                if let Some(e) = elem {
                    for (i, x) in a.iter().enumerate() {
                        walk(e, x, &format!("{p}[{i}]"), out);
                    }
                }
            }
            ("o", Some(x @ Value::Object(_))) => walk(elem.as_deref().unwrap_or(""), x, &p, out),
            (k, got) => out.push(format!("{p}: {k} expected, got {got:?}")),
        }
    }
}

/// Panics with every missing path.
pub fn assert_complete(class: &str, v: &Value) {
    let m = missing(class, v);
    assert!(
        m.is_empty(),
        "{} fields the client reads as null:\n{}",
        m.len(),
        m.join("\n")
    );
}
