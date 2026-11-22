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

### 2.3 Where the web view sits, and how big it is (H, Build 44)

- The Notices web view is `WebView_Info` (`scene_mypage`, path id 104), a `BaseWebView` with:
  alpha 1, background colour white, spinner on ("Loading"), vertical and horizontal scroll bars
  on, back button / bounces / zoom off, **immersiveMode on**, `DefaultScreenResolution`
  1280x720, offsets top 190, bottom 38, left 40, right 40.
- `BaseWebView.Start` 0x1074AE4 turns the offsets into UniWebView insets in device pixels:
  top/bottom × `DefaultScreenHeight/720`, left/right × `DefaultScreenWidth/1280`
  (`ScreenManager.Default*` = `Screen.width/height` read once at start, before
  `SetResolution`). For exactly 2160x1080, 2246x1080 and 2880x1440 it adds `73 × W/1280` to left
  and right (float at 0x2241240). The UI itself (CanvasScaler 1280x720, Expand) scales by the
  height on a wide screen and is centred, so the side insets only match the box on those sizes.
- Java side (`classes.dex`, `UniWebViewDialog.updateContentSize`): the web view is a dialog window
  at x = left, y = top, size = `displaySize()` − insets; with immersive mode on, `displaySize()`
  is `Display.getRealSize` (the whole panel, cutout included), not the area Unity draws in.
- Galaxy S22 (2340x1080, DPR 3; `shots-b43/notices_empty.jpg`): the game draws in about
  2246x1080 beside a ~94 px cutout strip, so Unity takes the 2246 case (left = right = 199 px,
  top 285, bottom 57), but the window is 2340 − 398 = 1942 px wide: about one cutout width
  wider than the box (inner frame ~1819 px, x 308..2128 on screen), so it runs past the box on
  one side. Inside, the page is 1942x738 px = about 647x246 CSS px.
- Web view settings: the plugin sets only JavaScript, DOM storage, database, file access and
  zoom controls off; no `setUseWideViewPort`, `setLoadWithOverviewMode`, `setTextZoom` or
  `setInitialScale`, and nothing in C# calls `UniWebView.SetUseWideViewPort`,
  `LoadWithOverviewMode` or `SetTransparentBackground`. So the page is laid out at the view's own
  CSS width, never zoomed out, and its own background covers the white web view.
- Our pages (Build 44): viewport `width=device-width, initial-scale=1, maximum-scale=1`; the
  root font size is `clamp(13px, min(2.5vw, 6.4vh), 22px)` and every other size is in em, so the
  type follows the view (about 16px in the S22 box, 22px on a tablet); `overflow-x` hidden on
  html/body, `overflow-wrap: anywhere`, images and tables at most 100% wide. Checked with
  Playwright at 607x247, 647x246, 731x246, 686x281, 960x524 and 420x200 CSS px (desktop and
  mobile viewport modes): no element past the view and no sideways scroll, also with a
  150-letter word and a 3000 px image.
- The window itself: `UniWebViewDialog` is a full-screen dialog with a transparent window
  (`ColorDrawable(0)`); the web view's own background is `BaseWebView.backgroundColor`, passed
  as `Color.argb(a×255, …)` to `setBackgroundColor`. Build 44 sets its alpha to 0 with a scene
  edit (`ui_edits.json` `set`: `scene_mypage:11:104` `backgroundColor.a` 0), and the page paints
  only `main`: `html`/`body` are transparent and `main` is inset on both sides by `--edge`, which
  the page script sets to the view's width minus the box the game designed for it (1280 − 40 − 40
  of a 1280x720 layout = 5/3 of the screen height; `min(screen.width, screen.height)` in case a
  web view reports the portrait size), at most a fifth of the view. On the S22 that is
  647 − 600 = 47 CSS px, so the painted page spans 47..600 of the 647 px view and stays inside the
  box whether the extra strip is on the left or on the right; the strip shows the game's box
  through it. On a 16:9 phone or a 4:3 tablet `--edge` is 0 (the window is the box). Checked with
  Playwright (screen and view sizes of the S22, a 1920x1080 and a 2400x1080 phone and a 2048x1536
  tablet; the S22 render laid over `notices_empty.jpg` at both possible window positions).
- Not run on a device: the transparent background, and whether the dialog's x is counted from
  the panel edge or the cutout edge (either way the painted page is inside the box). If the
  alpha edit did not take, the strip would show white instead of the game.
- Not fixed: the window's own size. A client code patch could size it to the box (right inset +=
  real width − Unity width), but the page and the transparent strip already keep what is drawn
  inside the box.

### 2.4 Build 45 correction (device screenshot of Build 44)

- Measured on the S22 screenshot: the web view window spans device x 98–2297 and y 284–1021
  (2199 × 737 px); the box's inner area is about x 310–2127. The window starts at the camera-cutout
  edge and ends 43 px before the panel edge, so the box centre sits about 21 px right of the
  window centre.
- Build 44 inset the page by the whole extra width on each side (twice too much), and its
  `color-scheme: dark` made Chromium paint its dark default (#121212) around the page. Build 45
  insets by half the extra width plus 1.5 % of the box, and asks for a light scheme so the
  transparent web view shows the game around the page. Rendered at the measured window and laid
  over the empty-box screenshot: the page sits inside the box frame.

