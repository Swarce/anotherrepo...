//! Real-data test: every object in the game's UI bundles and text.jp decodes through its own type
//! tree and encodes back to the same bytes, and every file re-serializes identically.
//! Needs JDR_DATA (the game's files/ folder) and JDR_KEYS (local keys.json).

use jdr_crypto::{names, Jjec};
use jdr_unity::{bundle, sfile};

#[test]
fn ui_bundles_and_text_round_trip() {
    let (Ok(dir), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let k = jdr_core::settings::Settings::load_keys(std::path::Path::new(&keys)).unwrap();
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    let mut files = 0;
    let mut objects = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        let Some(name) = fname
            .strip_suffix(".bytes")
            .and_then(|s| names::deobfuscate(s, key))
        else {
            continue;
        };
        if !(name.starts_with("ui_") && !name.starts_with("ui_tips")
            || name.starts_with("scene_")
            || name == "text.jp")
        {
            continue;
        }
        let mut b = std::fs::read(e.path()).unwrap();
        if Jjec::is_jjec(&b) {
            b = Jjec::decrypt(&b, key, iv).unwrap();
        }
        for n in bundle::read(&b)
            .unwrap()
            .iter()
            .filter(|n| n.flags & 4 != 0)
        {
            let f = sfile::parse(&n.data).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                f.write() == n.data,
                "{name}: file re-serializes identically"
            );
            for o in &f.objects {
                let nodes = &f.types[o.type_index as usize].nodes;
                let v = sfile::read_object(nodes, &o.data)
                    .unwrap_or_else(|e| panic!("{name} object {}: {e}", o.path_id));
                assert!(
                    sfile::write_object(nodes, &v).unwrap() == o.data,
                    "{name} object {} encodes back identically",
                    o.path_id
                );
                objects += 1;
            }
            files += 1;
        }
    }
    eprintln!("{files} serialized files, {objects} objects round-tripped");
    assert!(files > 50 && objects > 10_000);
}
