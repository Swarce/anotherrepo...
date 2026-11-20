//! Real-data test (JDR_APK): run `cleartext` and `debloat` on the player's APK in memory, then
//! read it back: libil2cpp.so differs only at the stubbed entry points, the manifest has lost
//! exactly the listed SDK elements and kept the game's own, and only the four SDK libraries are
//! gone.

use jdr_patch::apk::ApkEdit;
use jdr_patch::axml::Axml;
use jdr_patch::patches::{
    self, stub_bytes, Config, Patch, DEBLOAT_ELEMENTS, DEBLOAT_LIBS, LIBIL2CPP_ARM64, MANIFEST,
    MOV_W8_0,
};

#[test]
fn debloats_the_apk() {
    let Ok(apk_path) = std::env::var("JDR_APK") else {
        eprintln!("skipped: set JDR_APK");
        return;
    };
    let mut apk = ApkEdit::open(std::path::Path::new(&apk_path)).unwrap();
    let build = patches::identify(&mut apk).unwrap();
    let lib0 = apk.read(LIBIL2CPP_ARM64).unwrap();
    let names0 = apk.names();
    let cfg = Config::default();
    patches::Cleartext.apply(&mut apk, &cfg, build).unwrap();
    let man0 = Axml::parse(&apk.read(MANIFEST).unwrap())
        .unwrap()
        .elements();
    let detail = patches::Debloat.apply(&mut apk, &cfg, build).unwrap();
    eprintln!("{detail}");

    // libil2cpp.so: only the stub sites changed, to the stubs.
    let lib = apk.read(LIBIL2CPP_ARM64).unwrap();
    assert_eq!(lib.len(), lib0.len());
    let mut sites: Vec<(usize, Vec<u8>)> = build
        .sdk_stubs
        .iter()
        .map(|&(o, _, _, s)| (o, stub_bytes(s)))
        .collect();
    sites.push((
        build.title_play_games_button.0,
        MOV_W8_0.to_le_bytes().to_vec(),
    ));
    let mut expect = lib0.clone();
    for (o, b) in &sites {
        expect[*o..*o + b.len()].copy_from_slice(b);
    }
    assert!(lib == expect, "libil2cpp.so changed outside the stub sites");

    // Manifest: the listed elements are gone, with their children; the game's own are kept.
    let man = Axml::parse(&apk.read(MANIFEST).unwrap())
        .unwrap()
        .elements();
    for e in DEBLOAT_ELEMENTS {
        assert!(
            !man.iter().any(|(t, v)| (t.as_str(), v.as_str()) == *e),
            "{e:?}"
        );
    }
    for keep in [
        (
            "activity",
            "com.google.firebase.MessagingUnityPlayerActivity",
        ),
        ("activity", "com.bandainamcoent.jojodr.UnityPlayerActivity"),
        ("activity", "com.prime31.GoogleIABProxyActivity"),
        ("receiver", "jp.co.spchun.localpushlib.NotifyReceiver"),
        ("action", "android.intent.action.MAIN"),
        ("category", "android.intent.category.LAUNCHER"),
        ("uses-permission", "android.permission.INTERNET"),
        ("uses-permission", "android.permission.ACCESS_NETWORK_STATE"),
        ("uses-permission", "com.android.vending.BILLING"),
        ("meta-data", "unityplayer.UnityActivity"),
    ] {
        assert!(
            man.iter().any(|(t, v)| (t.as_str(), v.as_str()) == keep),
            "kept {keep:?}"
        );
    }
    // Every element left was there before, in the same order.
    let mut it = man0.iter();
    for e in &man {
        assert!(it.any(|x| x == e), "{e:?} new or reordered");
    }
    eprintln!("manifest: {} -> {} elements", man0.len(), man.len());
    if std::env::var("JDR_SHOW").is_ok() {
        for e in &man {
            eprintln!("  {e:?}");
        }
    }

    // Files: exactly the four SDK libraries are gone.
    let gone: Vec<&String> = names0.iter().filter(|n| !apk.exists(n)).collect();
    assert_eq!(gone.len(), DEBLOAT_LIBS.len(), "{gone:?}");
    for l in DEBLOAT_LIBS {
        assert!(!apk.exists(l));
    }
}
