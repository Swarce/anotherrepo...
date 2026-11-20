# 31 — The battle camera, and the menu cameras (Build 33)

H = read in the 3.12.0 disassembly or the player's files, F = seen in frames of original footage
(the owner's video research package, 77 observations from 17 videos, private, under
`local/wiki-raw/research-b33/video-local/`), M = inferred. Code:
`tools/content/scan_camera_requests.py`, `tools/content/build_camera_sets.py`,
`server/content/masterdata/Set_el_camera_set_common.json`, `Node_el_camera_node_common.json`.

## 1. The original (F)

- **Input phase:** one fixed default view, side-on and slightly 3/4, allies on the left third and
  enemies on the right, at chest height. No idle movement.
- **Every action, ally or enemy, cuts away** for about 1.5–3 s: the camera closes in on the
  attacker and cuts 2–4 times. In the full 2 fps frames (the owner's second package: X9xSMXBpsQw
  0:56–1:07 and 5:12–5:18, ZCUC-PwLXxA 3:49–3:52) every cut stays on the side the default view
  watches from: side-on to three-quarters behind the attacker, at chest height or a little lower,
  attacker and target framed from the knees or waist up at about 3–5 m, the attacker in the
  foreground. None looks from in front of the attacker or from beyond the target (the first
  package's notes said "reverse angle"; the frames do not show one). Big area finishers land in a
  high, wide view. Then a ~0.5 s settle back into the default view.
- **SP skills:** an 8–13 s cinematic on its own stage, or a ~3.5 s cut-in, then the hit on the
  field.
- **Time stop:** a cyan negative wave, then the field in blue-grey monochrome (HUD in colour).
- **Reversal:** a caption and starburst; the camera does not cut.
- **Boss intro:** low on the legs, a crane up the body to a tilted low full-body shot with the name
  card, about 2 s.
- **Stage finish:** the camera on the ground beside the downed boss, then CLEAR!!.
- **Win pose:** a low chest-up shot with a ~0.5 s push-in, held for about 4–5 s.

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
- **Later nodes are deltas.** `ScriptCamera.CreateNodeParam` 0xD70D44 builds step i's pose from a
  copy of the first step's node plus the `rotate`, `length`, `perspective`, `baseOffset` and
  `lookatOffset` of every later node up to i; only the bones come from node i itself.
- Bones: `CharacterBoneList.Bone` WAIST 1, SPINE2 4, NECK 5, HEAD 6 (docs/research/12 §5).

## 3. What we send (ours, Build 33)

- Before: every id played one world-fixed side view 9 m away (Build 13), so every action looked
  like the input phase: the "locked" camera the owner reported.
- Now (`build_camera_sets.py`): each requested id plays one of the footage's shots, framed on the
  acting unit, each with a slow push-in, all on the watched side:
  - a side-on two-shot, three-quarters behind the attacker, a closer side shot, a slightly low side
    shot, and a high wide view for the 14xxxx family;
  - picked by the role of the states that request the id (wind-up → side/back/close, hit →
    side/low/close, loop → side/back, wrap-up → side/close), varied by id.
- The client mirrors only ids 100400–100999 and 110000–149999 for enemies
  (`GameConst.IsMirroringCameraId` 0xDB66E4, ranges set in its constructor, H). Those are 2,250 of
  the 2,545 requests. The few other action ids (102xxx, 150000/150001) get a shot straight behind
  the attacker, which looks the same for either team; the boss intro (201, and 202/204/100201 that
  the boss's entrance appeal requests) and the stage finish are always an enemy and are framed for
  that side.
- The first Build 33 shots (over the shoulder, a front close-up, a reverse from beyond the target, a
  near-ground angle) put 74 poses on the far side of the arena even after the delta fix.
- The win shots: low, chest up, a 0.5 s push-in, held. The boss intro: low on the legs, a
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

## 6. Open (M)

- The SP cinematics depend on animator assets in the player's files and on the skill rows; whether
  ours trigger them is not checked yet.
- The Focus mode's parameters (JjsCameraParameter) and the 0.5 s settle are the client's.
