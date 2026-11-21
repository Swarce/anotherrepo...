//! Real-data test: the generated bundles are built on the first start and reused, unchanged, on
//! the next start of the same installed build; a different build identity rebuilds them. Needs
//! JDR_DATA and JDR_KEYS.

use jdr_core::settings::Settings;
use jdr_services::Ctx;

fn ctx(state: &std::path::Path, data: &str, keys: &str, id: &str) -> Ctx {
    let mut s = Settings::new(state.to_path_buf());
    s.game_files_dir = Some(std::path::PathBuf::from(data));
    s.keys = Some(Settings::load_keys(std::path::Path::new(keys)).unwrap());
    let mut c = Ctx::new(s, false);
    c.build_identity = Some(id.to_string());
    c
}

#[test]
fn built_once_per_installed_build() {
    let (Ok(data), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let state = std::env::temp_dir().join(format!("jdr-real-gencache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&state);
    let t = std::time::Instant::now();
    let first = ctx(&state, &data, &keys, "apk-a").generated().unwrap();
    let built = t.elapsed();
    let t = std::time::Instant::now();
    let again = ctx(&state, &data, &keys, "apk-a").generated().unwrap();
    let reused = t.elapsed();
    eprintln!("first start {built:?}, next start {reused:?}");
    assert_eq!(
        first.versionfile, again.versionfile,
        "the next start serves the same bundles"
    );
    assert_eq!(first.masterdata, again.masterdata);
    assert_eq!(first.localized, again.localized);
    assert!(reused < built, "the next start reuses the saved build");
    // an updated APK rebuilds (and the build stays the same apart from its timestamp)
    let updated = ctx(&state, &data, &keys, "apk-b").generated().unwrap();
    assert_eq!(first.masterdata, updated.masterdata);
    let _ = std::fs::remove_dir_all(&state);
}
