# 24 — Supporters and notices (Build 25)

Read in the 3.12.0 client: H = read in the disassembly, M = inferred. Code:
`server/services/src/services/support.rs`, `services/bizarre.rs` (the battle), `services/menu.rs`
(`notify_list`), `server/services/src/web.rs` (the pages).

## 1. Supporters

### 1.1 Client facts

- **Routes (H).** `/support/support_user_s {dummyFlg, stageType}` → `supportUserList`, a list of
  `{supportUserInfo}`. A `SupportUserInfo` is `{id, username, userRank, lastLoginAt,
  followStatus, statueInfoSupport {statueId, level, convergence}, friendPoint,
  leaderAbilityInfo, dtLeaderAbilityInfo, supportSkillName}`. The pick goes back as
  `bizarre_start.supportId`; the start reply's `supportUserInfo` is a UserInfoMini (`statueInfo`
  is a whole StatueInfo).
- **Tactical battle (H).** `BattleManager.OnSceneLoadCallback` 0xEBC330 takes the scene data's
  `SupportUserInfo` and uses its `statueInfo` only when `followStatus` is FollowEach (3)
  (0xEBCB2C). That statue goes to `SetupCharacterInfo` 0xBBBC30 → `CharacterManager.
  CreateCharacter` 0xEDCBA8, which passes its `dtLeaderAbilityInfo` (StatueInfo +0xD8) to every
  member as the support leader ability (`CharacterStatus.SetStatueInfo` 0xEFC570). This is the
  research pack's `party.friend_tactical` ("friend support grants a passive skill in Tactical
  Battle").
- **Other readers (H).** The pause screen lists the support (`BattleStatueAttributeUIController.
  CreateBattlePlayerListForPauseUI` 0xD36290, the one caller of `get_selectedSupportMemberInfo`).
- `{}` as `supportUserInfo` means no support (every story battle has sent it so since Build 06).

### 1.2 Server (ours)

- Five AI supporters a day per player (seeded by the player id and the game day, so the list is
  stable until 04:00 JST): five characters among the team members with a tactical leader ability
  (never assist-only statues), at the player's rank ±5 (Lv10–99), all FollowEach, named with the
  AI handles the rankings use.
- The supporter id carries its statue and level (`sup-<n>-<statueId>-<level>`), so a pick made
  just before the day turned still starts; any other `supportId` means no support.
- `bizarre_start` sends the supporter as a UserInfoMini with its whole statue (its
  `dtLeaderAbilityInfo` is the effect) and `leaderAblityId`; the pending battle remembers the
  pick, so a resumed battle keeps it.
- A clear with a supporter pays 10 more Friend Points (`friendPoint` on the list, and
  `mindGashaPoint` on the result; ours: the wiki gives 10 for a clear without a friend).
- Labyrinth battles take no supporter (their flow never asks for one).

### 1.3 Not built / unverified

- The action battle's callable support (`quest.support`, `quest.support_finisher`): the action
  battle is not served.
- Following, follower lists and friend requests stay empty (no other players).
- Not run on a device: the support list screen with AI entries, the passive in battle and the
  pause screen's support row.

## 2. Notices

### 2.1 Client facts (H)

- `/info/notify_list` → `{updateUrl, failureUrl, otherUrl, updateNewFlg, failureNewFlg,
  otherNewFlg}`: the Notices screen's three tabs (Update, Fault, Other;
  `InformationUIController`). `LoadPage` 0x203E5F0 hands the URL as it is to
  `BaseWebView.Load` 0x1075154 → `UniWebView.Load`: no base is added, so the URLs are absolute.
- The web view is the one the terms page and Campaign Info already use (our `/web` module).
- `/info/check_forced_notify {}` → `category` (ForceNotifyType: None 0, UpdateInfo 1, FaultInfo 2,
  OtherInfo 3) can open a tab by itself; we keep 0.
- `/info/banner_list` → `bannerList` (BannerInfo: art name, `signboardNextScene`, `htmlLink`,
  …): My Page signboards need banner art from the player's files; none says "what's new", so the
  list stays empty.

### 2.2 Server (ours)

- Build 42: the three tabs are pages of collapsible posts, one data file for all of them:
  `server/content/news.json` (our words; format in docs/CONTENT.md "Notices"), rendered by
  `web.rs` (`notice_page`). Update tab `<base>/web/information/build_notes.html`: one post per
  build, newest first (a post with `build` above `jdr_core::BUILD` is not shown yet). Fault tab
  `maintenance.html`: the offline "no maintenance" post. Other tab `about.html`: what the offline
  build includes, and the about post.
- Layout: `article.notice > button.headline (time.date, span.title) + section.content`, our own
  CSS (`server/content/notice.css`, every shape drawn in CSS; no retail notice images or
  stylesheet) and a small inline script, so a page loads no other file and names no URL. Rich
  text keeps only `<color=#RRGGBB>`, `<b>`, `<i>` and line breaks; anything else is shown as text.
- The page is sent with every post open; the script folds all but the first and toggles a post on
  a tap. So with scripts off the page still reads, all expanded. The client's web view runs
  scripts (H): `com.onevcat.uniwebview.UniWebView.<init>` in the APK's `classes.dex` calls
  `WebSettings.setJavaScriptEnabled(true)`, and the C# side (`UniWebView`, dump.cs) has no call
  that turns it off.
- `updateNewFlg` and the Home badge (`get_menu_notify.unopenedNotify`) are 1 until the player
  opens the Notices once on this build (`Progress.notes_seen_build`). Kept per build, not per
  post: news.json is built into the server, so new posts only arrive with a new build.
- Not run on a device: the three tabs loading our pages, and the tap toggling a post.
