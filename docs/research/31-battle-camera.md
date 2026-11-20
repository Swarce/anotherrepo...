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
  attacker and cuts 2–4 times (low close-up, over the shoulder, from behind the target looking
  back, reverse angle). Big area finishers land in a high, wide view. Then a ~0.5 s settle back into
  the default view.
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
  them their positions are taken once. A set's `dataList` steps play in turn, each blending in over
  `inter` seconds; the last pose is held.
- Bones: `CharacterBoneList.Bone` WAIST 1, SPINE2 4, NECK 5, HEAD 6 (docs/research/12 §5).

## 3. What we send (ours, Build 33)

- Before: every id played one world-fixed side view 9 m away (Build 13), so every action looked
  like the input phase: the "locked" camera the owner reported.
- Now (`build_camera_sets.py`): each requested id plays one shot of the footage's grammar, framed
  on the acting unit and mirrored for enemies, each with a slow push-in:
  - over the shoulder, a low front close-up, the reverse from the target's side, a side two-shot,
    a near-ground angle, a pose close-up, and a high wide view for the 14xxxx family;
  - picked by the role of the states that request the id (wind-up → shoulder/low/pose, hit →
    reverse/side/ground, loop → side/reverse/ground, wrap-up → side/pose/shoulder), varied by id.
- The win shots: low, chest up, a 0.5 s push-in, held. The boss intro (201): low on the legs, a
  crane up to a tilted full-body shot. Stage finishes: a low shot beside the downed unit.
- The boss intro is on again for the boss of each chapter's last stage (the first enemy of its last
  area); it had been off since Build 18 for want of a working camera.
- Camera heights were checked numerically against a 1.8 m model: all above the floor.

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
