//! Real-data tests: run only when JDR_DATA points at the player's own `files/` folder.

use jdr_crypto::{keyrec, names, Jjec, Vcs};
use std::path::PathBuf;

fn data_dir() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("JDR_DATA").ok()?);
    p.is_dir().then_some(p)
}

#[test]
fn recover_keys_and_decrypt_every_bundle() {
    let Some(dir) = data_dir() else {
        eprintln!("skipped: set JDR_DATA to the game's files/ folder");
        return;
    };
    let mut files = vec![];
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if let Some(raw) = p
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(names::unhex)
        {
            files.push((p, raw));
        }
    }
    let raws: Vec<Vec<u8>> = files.iter().map(|f| f.1.clone()).collect();
    let nk = keyrec::recover_name_key(&raws).expect("key");
    assert!(nk.score > 0.999, "score {}", nk.score);

    let mut iv = None;
    let (mut jjec, mut ok) = (0, 0);
    for (p, raw) in &files {
        let d = std::fs::read(p).unwrap();
        // every name decodes to the expected alphabet
        let n = names::xor(raw, &nk.key);
        assert!(
            n.iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.".contains(c)),
            "{}",
            p.display()
        );
        if Jjec::is_jjec(&d) {
            jjec += 1;
            let iv = *iv.get_or_insert_with(|| keyrec::recover_iv(&nk.key, &d).expect("iv"));
            let plain = Jjec::decrypt(&d, &nk.key, &iv).unwrap();
            if plain.starts_with(b"UnityFS\0") {
                ok += 1;
            }
        }
    }
    assert!(jjec > 0);
    assert_eq!(ok, jjec, "every JJEC file must decrypt to a UnityFS bundle");

    let vcs = std::fs::read(dir.join("vcs.bytes")).unwrap();
    let (_size, _md5, plain) = Vcs::decrypt(&vcs, &nk.key, &iv.unwrap()).unwrap();
    assert!(plain.starts_with(b"{\""), "vcs payload is JSON");
    eprintln!(
        "{jjec} encrypted bundles decrypted, {} names decoded",
        files.len()
    );
}
