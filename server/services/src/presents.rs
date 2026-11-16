//! The Message Box (`present/*`, docs/research/18-message-box.md): gifts waiting to be received
//! (mission rewards, the welcome gift) and the received history. Saved with the story progress.
//!
//! Client rules this follows (3.12.0): `prevNum` is the 1-based page; a row shows in the tab whose
//! `receivedFlag` equals its `notify`; `dataId` must parse as a number for dataType 3–6 (the row
//! drawing throws otherwise); `name` is a label key and `comment` is literal text in which
//! `{LABEL}` is replaced by that label's text.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Rows per page (the client has no page size; ours).
pub const PAGE_SIZE: usize = 20;
/// Received rows kept for the history tab (ours).
pub const HISTORY_KEEP: usize = 100;

/// PresentInfo `dataType`.
pub const DATA_ITEM: i64 = 3;
pub const DATA_STONE: i64 = 4;
pub const DATA_MONEY: i64 = 5;
pub const DATA_FRIEND_POINT: i64 = 6;

/// Present error codes (client ErrorCode).
pub const E_PRESENT_NOT_EXIST: i64 = 51401;
pub const E_PRESENT_ALREADY_GET: i64 = 51402;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Present {
    pub uid: i64,
    pub data_type: i64,
    pub data_id: i64,
    pub count: i64,
    /// Literal text; `{LABEL}` is replaced by the client.
    pub comment: String,
    pub created: i64,
    #[serde(default)]
    pub received: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresentBox {
    #[serde(default)]
    pub next_uid: i64,
    #[serde(default)]
    pub list: Vec<Present>,
    /// The one-time welcome gift was put in the box.
    #[serde(default)]
    pub welcomed: bool,
}

/// What a received present gives (the caller applies it to the account).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gift {
    pub data_type: i64,
    pub data_id: i64,
    pub count: i64,
}

/// The `dataType` and `dataId` for a reward item id as missions give them: 0/10 Diamond Coins,
/// 20 gold, 40 Friend Points, anything else a catalog item.
pub fn reward_kind(item_id: i64) -> (i64, i64) {
    match item_id {
        0 | 10 => (DATA_STONE, 10),
        20 => (DATA_MONEY, 20),
        40 => (DATA_FRIEND_POINT, 40),
        id => (DATA_ITEM, id),
    }
}

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

impl PresentBox {
    pub fn add(&mut self, data_type: i64, data_id: i64, count: i64, comment: String, now: i64) {
        self.next_uid += 1;
        self.list.push(Present {
            uid: self.next_uid,
            data_type,
            data_id,
            count,
            comment,
            created: now,
            received: None,
        });
    }

    /// Put the one-time welcome gift in the box (offline sandbox; amounts ours).
    pub fn welcome(&mut self, now: i64) -> bool {
        if self.welcomed {
            return false;
        }
        self.welcomed = true;
        self.add(
            DATA_MONEY,
            20,
            100_000,
            "Welcome back! A gift from the offline server.".into(),
            now,
        );
        self.add(
            DATA_ITEM,
            crate::items::MEMORY_RING,
            500,
            "Welcome back! Memory Rings for the Ring Exchange.".into(),
            now,
        );
        true
    }

    pub fn waiting(&self) -> usize {
        self.list.iter().filter(|p| p.received.is_none()).count()
    }

    /// Rows of one tab, newest first.
    fn tab(&self, received: bool) -> Vec<&Present> {
        let mut v: Vec<&Present> = self
            .list
            .iter()
            .filter(|p| p.received.is_some() == received)
            .collect();
        if received {
            v.sort_by_key(|p| std::cmp::Reverse((p.received, p.uid)));
        } else {
            v.sort_by_key(|p| std::cmp::Reverse(p.uid));
        }
        v
    }

    /// `{allPresentNum, allPageNum, presentList}` for a tab (0 waiting, 1 received) and a 1-based
    /// page. `allPageNum` is always at least 1 (the arrows need it); a page past the end shows the
    /// last one.
    pub fn page(&self, received_flag: i64, page: i64) -> Value {
        let rows = self.tab(received_flag == 1);
        let pages = rows.len().div_ceil(PAGE_SIZE).max(1);
        let page = (page.max(1) as usize).min(pages);
        let list: Vec<Value> = rows
            .iter()
            .skip((page - 1) * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|p| info(p))
            .collect();
        json!({ "allPresentNum": rows.len(), "allPageNum": pages, "presentList": list })
    }

    /// Receive one present (`Err` = the client's error code) or every waiting one.
    pub fn receive(&mut self, uid: Option<i64>, now: i64) -> Result<Vec<Gift>, i64> {
        let mut gifts = Vec::new();
        for p in self.list.iter_mut() {
            if uid.is_some_and(|u| u != p.uid) {
                continue;
            }
            if p.received.is_some() {
                if uid.is_some() {
                    return Err(E_PRESENT_ALREADY_GET);
                }
                continue;
            }
            p.received = Some(now);
            gifts.push(Gift {
                data_type: p.data_type,
                data_id: p.data_id,
                count: p.count,
            });
        }
        if uid.is_some() && gifts.is_empty() {
            return Err(E_PRESENT_NOT_EXIST);
        }
        // Keep the newest received rows only.
        let mut received: Vec<(i64, i64)> = self
            .list
            .iter()
            .filter_map(|p| p.received.map(|r| (r, p.uid)))
            .collect();
        if received.len() > HISTORY_KEEP {
            received.sort();
            let drop: std::collections::BTreeSet<i64> = received[..received.len() - HISTORY_KEEP]
                .iter()
                .map(|x| x.1)
                .collect();
            self.list.retain(|p| !drop.contains(&p.uid));
        }
        Ok(gifts)
    }
}

/// The label key naming what a present gives.
pub fn name(data_type: i64, data_id: i64) -> String {
    let _ = data_type; // items, coins, gold and Friend Points all use the item labels
    format!("ITEM_{data_id:04}_NAME")
}

/// PresentInfo as the row reads it. `notify` is the "received" flag the tabs filter on.
pub fn info(p: &Present) -> Value {
    json!({ "presentId": p.uid, "presentuId": p.uid, "messageType": 0, "dataType": p.data_type,
            "dataId": p.data_id.to_string(), "name": name(p.data_type, p.data_id), "count": p.count,
            "rare": 0, "notify": i64::from(p.received.is_some()), "comment": p.comment, "level": 0,
            "indefinitely": 1, "dueDate": crate::missions::NO_END, "getDate": date(p.created),
            "receiveDate": p.received.map(date).unwrap_or_else(|| date(p.created)), "itemInfo": [] })
}

/// The `getPresentList` history entry (the "received" dialog reads name and count).
pub fn gift_info(g: &Gift) -> Value {
    json!({ "dataType": g.data_type, "dataId": g.data_id.to_string(),
            "name": name(g.data_type, g.data_id), "count": g.count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_tabs_and_receiving() {
        let mut b = PresentBox::default();
        assert!(b.welcome(100));
        assert!(!b.welcome(200), "the welcome gift is given once");
        for i in 0..25 {
            b.add(DATA_ITEM, 6100, i + 1, format!("gift {i}"), 300 + i);
        }
        assert_eq!(b.waiting(), 27);
        let p1 = b.page(0, 1);
        assert_eq!(
            (p1["allPresentNum"].as_i64(), p1["allPageNum"].as_i64()),
            (Some(27), Some(2))
        );
        assert_eq!(p1["presentList"].as_array().unwrap().len(), PAGE_SIZE);
        assert_eq!(p1["presentList"][0]["comment"], "gift 24", "newest first");
        assert_eq!(p1["presentList"][0]["notify"], 0);
        assert!(p1["presentList"][0]["dataId"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .is_ok());
        assert_eq!(
            b.page(0, 9)["presentList"].as_array().unwrap().len(),
            7,
            "past the end = last"
        );
        assert_eq!(b.page(1, 1)["allPageNum"], 1, "an empty tab is page 1 of 1");

        let uid = p1["presentList"][0]["presentuId"].as_i64().unwrap();
        let g = b.receive(Some(uid), 1000).unwrap();
        assert_eq!(
            g,
            vec![Gift {
                data_type: DATA_ITEM,
                data_id: 6100,
                count: 25
            }]
        );
        assert_eq!(b.receive(Some(uid), 1001), Err(E_PRESENT_ALREADY_GET));
        assert_eq!(b.receive(Some(9999), 1001), Err(E_PRESENT_NOT_EXIST));
        assert_eq!(b.page(1, 1)["presentList"][0]["notify"], 1);

        let all = b.receive(None, 2000).unwrap();
        assert_eq!(all.len(), 26);
        assert_eq!(b.waiting(), 0);
        assert_eq!(b.receive(None, 2001), Ok(vec![]));
        assert_eq!(reward_kind(20), (DATA_MONEY, 20));
        assert_eq!(reward_kind(0), (DATA_STONE, 10));
    }
}
