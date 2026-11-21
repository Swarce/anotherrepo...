//! `/support/support_user_s` and the support of a quest battle: AI supporters
//! (docs/research/27-support-and-news.md).
//!
//! Client facts (3.12.0, H): the support list is `SupportUserResponse.supportUserList`, one
//! `SupportUserInfo` per entry (`statueInfoSupport` for the thumbnail, the leader abilities for the
//! effect line); the pick goes back as `bizarre_start.supportId`, and the start reply's
//! `supportUserInfo` (a UserInfoMini) is the support in battle. The tactical battle takes the
//! support's statue only when `followStatus` is FollowEach (3), and then applies that statue's
//! `dtLeaderAbilityInfo` to every member (`OnSceneLoadCallback` 0xEBC330 → `SetupCharacterInfo`
//! → `CreateCharacter` 0xEDCBA8, the support leader ability of `CharacterStatus.SetStatueInfo`).
//!
//! Ours: there are no other players offline, so the list holds AI supporters: five statues with a
//! tactical leader ability, a new set each game day (04:00 JST), at a level near the player's
//! rank, all mutual friends (FollowEach), each worth extra Friend Points on a clear.

use crate::accounts::Profile;
use crate::collection::{self, OwnedStatue};
use crate::services::ranking::{hash, Rng, HANDLES};
use serde_json::{json, Value};

/// Supporters offered per list (ours).
pub const SUPPORTERS: usize = 5;
/// Friend Points a clear with a supporter adds (ours; the wiki gives 10 for a clear without a
/// friend).
pub const SUPPORT_FRIEND_POINTS: i64 = 10;
/// FollowStatus.FollowEach (client): the only status whose support statue the tactical battle uses.
const FOLLOW_EACH: i64 = 3;

/// One AI supporter. Its id carries everything needed to rebuild it (`sup-<n>-<statueId>-<level>`),
/// so a pick made before the day turned still starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Supporter {
    pub id: String,
    pub n: usize,
    pub statue_id: String,
    pub level: i64,
}

impl Supporter {
    pub fn parse(id: &str) -> Option<Supporter> {
        let rest = id.strip_prefix("sup-")?;
        let (n, rest) = rest.split_once('-')?;
        let (statue_id, level) = rest.rsplit_once('-')?;
        let s = Supporter {
            id: id.to_string(),
            n: n.parse().ok()?,
            statue_id: statue_id.to_string(),
            level: level.parse().ok()?,
        };
        (collection::can_be_member(&s.statue_id) && (1..=collection::MAX_LEVEL).contains(&s.level))
            .then_some(s)
    }

    pub fn name(&self) -> String {
        format!(
            "{} {}",
            HANDLES[self.n % HANDLES.len()],
            100 + self.n * 37 % 900
        )
    }

    pub fn rank(&self) -> i64 {
        self.level.clamp(1, 200)
    }

    fn owned(&self) -> OwnedStatue {
        OwnedStatue {
            uid: 0,
            statue_id: self.statue_id.clone(),
            level: self.level,
            exp: collection::exp_for_level(self.level),
            ..Default::default()
        }
    }
}

/// Statues a supporter may lead with: team members with a tactical leader ability.
fn candidates() -> &'static [&'static collection::StatueDef] {
    static CELL: std::sync::OnceLock<Vec<&'static collection::StatueDef>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        collection::members()
            .filter(|d| collection::leader_ability(&d.statue_id) != json!({}))
            .collect()
    })
}

/// Today's supporters for a player: five different characters, a new set each game day, at the
/// player's rank give or take five levels (10..=99; ours).
pub fn supporters(user_id: &str, rank: i64, now: i64) -> Vec<Supporter> {
    let day = crate::missions::game_day(now);
    let mut rng = Rng(hash(user_id) ^ (day as u64).wrapping_mul(0x9e37_79b9));
    let pool = candidates();
    let mut out: Vec<Supporter> = Vec::new();
    for _ in 0..200 {
        if out.len() == SUPPORTERS || pool.is_empty() {
            break;
        }
        let d = pool[rng.below(pool.len())];
        if out.iter().any(|s| s.statue_id[..4] == d.statue_id[..4]) {
            continue;
        }
        let level = (rank + rng.below(11) as i64 - 5).clamp(10, collection::MAX_LEVEL);
        let n = out.len() + 1 + (day.rem_euclid(97) as usize) * SUPPORTERS;
        out.push(Supporter {
            id: format!("sup-{n}-{}-{level}", d.statue_id),
            n,
            statue_id: d.statue_id.clone(),
            level,
        });
    }
    out
}

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// `SupportUserInfo` for the support list (every string, list and object set).
pub fn support_user_info(s: &Supporter, now: i64) -> Value {
    json!({
        "id": s.id, "username": s.name(), "userRank": s.rank(),
        "lastLoginAt": date(now - 60 * (5 + s.n as i64 % 50)), "followStatus": FOLLOW_EACH,
        "statueInfoSupport": { "statueId": s.statue_id, "level": s.level, "convergence": 0 },
        "friendPoint": SUPPORT_FRIEND_POINTS,
        "leaderAbilityInfo": collection::action_leader_ability(&s.statue_id),
        "dtLeaderAbilityInfo": collection::leader_ability(&s.statue_id),
        "supportSkillName": ""
    })
}

/// The battle's `supportUserInfo` (a UserInfoMini): the supporter as a mutual friend, with its
/// whole statue (the battle reads its `dtLeaderAbilityInfo`).
pub fn battle_support_info(s: &Supporter, now: i64) -> Value {
    let statue = collection::statue_info(&s.owned());
    let leader = statue["dtLeaderAbilityInfo"]["abilityId"]
        .as_str()
        .unwrap_or("")
        .to_string();
    json!({
        "id": s.id, "username": s.name(), "userRank": s.rank(), "clanId": 0, "clanName": "",
        "position": 0, "comment": "", "lastLoginAt": date(now - 300), "followNum": 0,
        "followerNum": 0, "followStatus": FOLLOW_EACH, "statueInfo": statue, "equipMindList": [],
        "supportSkillId": "", "leaderAblityId": leader, "availableAt": "",
        "friendPoint": SUPPORT_FRIEND_POINTS
    })
}

/// `/support/support_user_s {dummyFlg, stageType}` → `{supportUserList}`: today's AI supporters
/// (the same list for both battle types; ours).
pub fn support_list(p: &Profile, now: i64) -> Value {
    let list: Vec<Value> = supporters(&p.user_id, p.rank.max(1), now)
        .iter()
        .map(|s| json!({ "supportUserInfo": support_user_info(s, now) }))
        .collect();
    json!({ "supportUserList": list })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supporters_are_five_characters_with_leader_abilities_and_round_trip() {
        let now = 1_800_000_000;
        let a = supporters("u1", 30, now);
        assert_eq!(a.len(), SUPPORTERS);
        let mut charas: Vec<&str> = a.iter().map(|s| &s.statue_id[..4]).collect();
        charas.dedup();
        assert_eq!(charas.len(), SUPPORTERS);
        for s in &a {
            assert!((25..=35).contains(&s.level));
            assert_ne!(collection::leader_ability(&s.statue_id), json!({}));
            assert_eq!(Supporter::parse(&s.id).as_ref(), Some(s));
            let info = battle_support_info(s, now);
            assert_eq!(info["followStatus"], FOLLOW_EACH);
            assert_eq!(info["statueInfo"]["level"], s.level);
            assert_eq!(
                info["leaderAblityId"],
                info["statueInfo"]["dtLeaderAbilityInfo"]["abilityId"]
            );
            // The list entry carries the same action leader ability as the battle's statue.
            assert_eq!(
                support_user_info(s, now)["leaderAbilityInfo"],
                info["statueInfo"]["leaderAbilityInfo"]
            );
        }
        assert_eq!(supporters("u1", 30, now), a, "stable within the day");
        assert_ne!(
            supporters("u1", 30, now + 86_400),
            a,
            "a new set the next day"
        );
        assert!(
            Supporter::parse("sup-1-9999_99-30").is_none(),
            "unknown statue"
        );
        assert!(Supporter::parse("nope").is_none());
    }
}
