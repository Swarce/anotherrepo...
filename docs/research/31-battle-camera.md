# 31 — The battle camera, and the menu cameras (Build 33)

H = read in the 3.12.0 disassembly or the player's files, F = seen in frames of original footage
(the owner's video research package, 77 observations from 17 videos, private, under
`local/wiki-raw/research-b33/video-local/`), M = inferred. Code:
`tools/content/scan_camera_requests.py`, `tools/content/build_camera_sets.py`,
`server/content/masterdata/Set_el_camera_set_common.json`, `Node_el_camera_node_common.json`.

## 1. The original (F)

- **Input phase:** one fixed default view, side-on and slightly 3/4, allies on the left third and
  enemies on the right, at chest height. No idle movement.
- **The action's kind decides the shots** (every frame read, docs/research/32 §1): normal attacks
  and blue skills stay near the default view, pushed in or as a side-on two-shot, sometimes with
  one low cut behind the attacker, often with no cut at all. Red (Stand) SP skills and time stops
  cut between angles on the battlefield: over the shoulder, a low front three-quarter, the reverse
  from the enemy side, a 90° side shot, a high wide view 45–60° down. All watch from the default
  view's side of the line between the teams. Then a ~0.5 s blur as the HUD returns.
- **SP skills:** an 8–13 s cinematic on its own stage, or a ~3.5 s cut-in, then the hit on the
  field.
- **Time stop:** a cyan negative wave, then the field in blue-grey monochrome (HUD in colour).
- **Reversal:** a caption and starburst; the camera does not cut.
- **Boss intro:** low on the legs, a crane up the body to a tilted low full-body shot with the name
  card, about 2 s.
- **Stage finish:** the camera on the ground beside the downed boss, then CLEAR!!.
- **Win pose** (full frames: ZCUC-PwLXxA 5:17–5:22 and 9:06, X9xSMXBpsQw 6:16–6:20): a tight
  head-and-chest shot from well below (about 25–30° up at the face), the winner right of centre
  and turned towards screen left, where WIN! and the quote strip sit. It opens closer and more
  turned, settles over about 1 s and holds for about 4 s.

## 2. How the client drives it (H)

- The tactical camera has modes (`Jjs.DTCameraMode`): Overview (the default view), Focus,
  StageFinish, ScriptCamera and Free; SP cinematics play animator-driven ("Maya") cameras.
- The statues' tactical action sets request script cameras with `DoChangeCamera` /
  `DoChangeCameraDelay` actions (`cameraType` 1 = script, `scriptCameraId`, `delaySec`): 2,545
  requests, 245 distinct ids, in families 110xxx/111xxx (attacks), 130xxx (skills), 131xxx–132xxx,
  14xxxx (big moves), 101xxx/301 (result). Ids 1000–2999 are requested with 100000 added.
- `DoChangeCamera.ChangeCameraDtBattle` 0xD0007C calls `SetScriptCameraInfo(id, tgt0, null, null,
  null, rotateInverse, holdFinalPose = true)`: one target, the acting unit (its Stand for a pair
  action); `rotateInverse` for the enemy team.
- `ScriptCamera.CreateNodeParam` 0xD70D44 negates `rotate.y` and the x offsets when mirrored.
- `ScrptCameraUpdate` 0xD6EDAC: the camera stands at bone(base) + R × Euler(rotate) × (0, 0,
  length) and looks at bone(lookat) + R × lookatOffset; R is the unit's rotation unless
  `flagWorldSet`. With `flagBindBase`/`flagBindLookat` the bones are followed every frame, without
  them their positions are taken once. A set's `dataList` steps play in turn: during step i the
  camera moves from step i's pose to step i+1's over step i's `sec`; the last pose is held.
- **`inter` moves the camera** (`ScriptCamera.SetCameraInfo` 0xD6FE60): every frame the camera goes
  to `Lerp(current position, computed position, inter)` and the field of view to
  `Lerp(current, computed, inter)`; the rotation is `LookRotation(look-at − position)` with the roll
  lerped by `inter`. `inter` is the current step's, a per-frame share (not scaled by time): 0 never
  moves the camera, 1 cuts and follows exactly; the client's sample sets
  (`ScriptCameraTestData.CreateSetAsset` 0xD7236C) and the step constructor use 0.3. `beginInter`
  only matters when exactly 1.0 (a first-frame snap). Builds 33–36 sent 0 on every step.
- **Later nodes are deltas.** `ScriptCamera.CreateNodeParam` 0xD70D44 builds step i's pose from a
  copy of the first step's node plus the `rotate`, `length`, `perspective`, `baseOffset` and
  `lookatOffset` of every later node up to i; only the bones come from node i itself.
- Bones: `CharacterBoneList.Bone` WAIST 1, SPINE2 4, NECK 5, HEAD 6 (docs/research/12 §5).

## 3. What we send (ours, Build 33)

- Before: every id played one world-fixed side view 9 m away (Build 13), so every action looked
  like the input phase: the "locked" camera the owner reported.
- Now (`build_camera_sets.py`): each requested id gets its shots from the kind of action that
  requests it, read from the action set's template name (`…_ATTACK`, `…_SKILL_NN`, the skill
  red when the statue's tactical skill is special; `scan_camera_requests.py` records the template):
  - attacks and blue skills (Build 35, measured on X9xSMXBpsQw 113-133, ZCUC-PwLXxA 460-920, X9xS
    628 and checked by rendering posed models): the main shot on the turn camera's side 45° off the
    attacker's facing, about 1.1 m up and level, 5 m away (the attacker at the left third head to
    feet, the target nearer at the right), a wider 60° variant, and over the attacker's shoulder
    3.8 m back; 36° field of view. The Build 33 shots were perpendicular or behind, 2.6–4.6 m away
    and 1.6–2.0 m up;
  - red SP skills (an id gets these when SPs are at least a third of its users): over the shoulder,
    a low front three-quarter, the reverse from beyond the target, the main two-shot, a high wide
    view 50° down (the 14xxxx family's hits);
  - picked by the role of the requesting states (wind-up, hit, loop, wrap-up) and varied by id.
  - Camera id families do not separate the kinds: 130xxx alone is requested by 554 SP, 502 skill
    and 466 other skill states.
- The client mirrors only ids 100400–100999 and 110000–149999 for enemies
  (`GameConst.IsMirroringCameraId` 0xDB66E4, ranges set in its constructor, H). Those are 2,250 of
  the 2,545 requests. The few other action ids (102xxx, 150000/150001) get a shot straight behind
  the attacker, which looks the same for either team; the boss intro (201, and 202/204/100201 that
  the boss's entrance appeal requests) and the stage finish are always an enemy and are framed for
  that side.
- The first Build 33 shots (over the shoulder, a front close-up, a reverse from beyond the target, a
  near-ground angle) put 74 poses on the far side of the arena even after the delta fix.
- The win shot (Build 37, from the owner's screenshot of the original): still, 2.4 m from the
  chest, 5° below, 16° to the side, the winner about 70 % across and framed to the upper thighs.
  Build 35 had turned it 180°, read from the Build 34 recording where the camera never moved
  (`inter` 0) and only turned to the winner; that is undone.
- The win shots (third package): 1.15 m from the chest, 26° below, 27° to the side, the look
  point offset so the winner sits right of centre; opening at 1.0 m, 30° and 34° and settling over
  1 s, then held (rendered with the statues' own win-appeal clip, `pose_models.py --clip
  evt_btl_lp`). The first Build 33 shot was 2.05 m away and 8° below, head to waist and centred.
  Very small models (Iggy, Pet Shop) put the camera at or under the floor in this shot. The boss intro: low on the legs, a
  crane up to a tilted full-body shot. Stage finishes: a low shot beside the downed unit.
- The boss intro is on again for the boss of each chapter's last stage (the first enemy of its last
  area); it had been off since Build 18 for want of a working camera.
- **Build 33 device test:** most cameras stood very far away, under the ground or away from the
  character. The first Build 33 package sent each push-in's end pose as an absolute node, so the
  client added it to the start pose: distance, angles and field of view doubled on the second step
  (tripled on the win shot's third). A simulation of the client's code on that data gives cameras
  down to 5.4 m under the floor, up to 17 m away and fields of view up to 102°.
- Now each set sends its start node and one delta node (`<shot>_to_b`); the push-in runs over the
  first step's `sec` and the end pose is held. The same simulation, for allies and enemies (mirrored
  only where the client mirrors) on a 1.8 m model, keeps every pose above the floor, on the watched
  side, within 9.2 m and at 30–36°; a test checks the summed distance, field of view and pitch of
  every step.

## 5. The team screen and My Page (F, H)

Neither screen uses a script camera; both place the statue in front of a fixed camera, so the
framing comes from the statue rows we send (`statue_setting_table`, docs/research/11 §1).

- **Team screen (F).** The original (2017 and 2019 footage) shows the three statues thighs-up, each
  centred over its card, heads at about the same height in every slot, the leader as large as the
  others. Ours (H for the camera and anchors) had the leader's head clearly lower for every short
  model: the leader stands nearer the camera, and we lifted heads only to 1.45 m. Every head is now
  lifted to 1.68 m (`build_statues.py` `team_frame`; docs/research/12 §7).
- **My Page (F).** The leader stands right of centre (about two thirds across), framed from mid-thigh
  up, the head in the upper quarter, turned slightly to screen left; the camera does not move. Read
  against the client's camera (the origin, 40°): about 1.8 m away, 0.35 m right, the head bone a
  quarter metre above the camera. Ours stood 3.2 m away with the whole body down to the feet in view.
  `myPagePos` is now per statue: (35, 25 − head height, 180) cm, the head height from the team pose
  but at least 80% of the model's standing height (a crouching team pose would sink it).
- The original hid a statue's Stand on My Page (2019, Jotaro); that is the client's choice of model.

## 7. Every tactical action measured (Build 38)

Six readers went through every tactical action of six original videos (ZCUC, p1zD, X9xS, x0kb, WAwk,
h6Hs: 223 actions) frame by frame, aligning the background between frames (ORB + RANSAC) to tell a
cut from a glide; a mapping stage aligned each action with the camera ids its action set requests
(F; the measurements are private, under `local/wiki-raw/research-b33/camera-pack/`).

- **Normal attacks** (59 with a known ATTACK template): no cut in 54 (92 %). One shot: a glide
  along the turn view's line towards the target (85–96 % done within 0.5 s, the tilt levelled),
  a hold, then a 1 m truck with a 10–15 % pull-out over 1–2.5 s. The "behind the attacker" frames
  of the earlier notes (ZCUC 463, 900) are the attack animation turning its back; the background
  does not move. Return: an ease of 1–1.5 s; none when the kill ends the area.
- **Enemy attacks** (58): named units with an ATTACK template glide like ally attacks (0 cuts in
  33 of 35); generic gang thugs' attacks are skill sets that cut once to a low party-side shot and
  back (2 cuts).
- **Blue skills** (52): 3.5 cuts on average (median 3), an angle held 0.5–1 s, rushes cutting every
  0.5 s.
- **Red SPs** (25): 5.7 cuts (median 5): 1–3 close lead-in shots, black, the cinematic, a wipe, 2–4
  field shots, a cut near the turn view.
- **Time stop** (2019): a glide push-in; the cuts fall inside the flash frames. **Reversal and
  reserve swaps**: no camera change.
- **Win**: static in world space while the character animates (all 7 instances).
- The engine reproduces a glide with `inter` 0.09 (about 93 % of the way in 0.5 s) and a cut with
  1.0. `build_camera_sets.py` gives the 30 ids observed their measured shot (111000 the attack
  glide, 131000 the Stand attack glide, 130002 the cut near the turn view, 130005 the reverse,
  130010 the near-top-down view, the 1308xx/1309xx rushes, …) and every other id a shot by the kind
  of action and the role of its states with the measured frequencies. Simulated over all action
  sets: normal attacks 0.02 cuts (85 of 86 none), blue skills 3.2 (median 3), SPs 3.1 field cuts
  plus the client's cinematic wipes.

## 6. Open (M)

- The SP cinematics depend on animator assets in the player's files and on the skill rows; whether
  ours trigger them is not checked yet.
- The Focus mode's parameters (JjsCameraParameter) and the 0.5 s settle are the client's.
