//! Recover the asset key and IV from the player's own downloaded files.
//!
//! The server used to hand out `assetkey`/`assetvector` at every boot (`/client/getkey`) and the
//! client never stored them. Two facts let us get them back from the player's copy alone:
//!
//! 1. Bundle file names are XORed with the same 16-byte asset key ([`crate::names`]). Real names
//!    only use `[a-z0-9_.]`, so for each key position the right byte is the one that makes almost
//!    every name decode into that alphabet (a frequency attack; no plaintext list needed).
//! 2. JJEC bundles are AES-128-CBC. With the right key and a wrong IV only the first block is
//!    wrong, and the first block of every Unity 2018 bundle is the fixed header
//!    `"UnityFS\0" 00 00 00 06 "5.x."`, so `IV = AES_dec(C0) xor P0`.

use crate::aescbc;
use crate::container::{Jjec, JJEC_HEADER};
use crate::names;

pub const KEY_LEN: usize = 16;
/// First 16 bytes of a Unity 2018.x UnityFS bundle (format constant, not game data).
pub const UNITYFS_BLOCK0: &[u8; 16] = b"UnityFS\0\0\0\0\x065.x.";

fn in_alphabet(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'.'
}

#[derive(Debug, Clone, PartialEq)]
pub struct NameKeyResult {
    pub key: [u8; KEY_LEN],
    /// Fraction of all name bytes that decode into the alphabet (1.0 = perfect).
    pub score: f64,
    pub names_used: usize,
}

/// Frequency attack on obfuscated name stems (raw bytes, already un-hexed).
pub fn recover_name_key(stems: &[Vec<u8>]) -> Option<NameKeyResult> {
    if stems.len() < 50 {
        return None;
    }
    let mut key = [0u8; KEY_LEN];
    for (i, slot) in key.iter_mut().enumerate() {
        let mut best = (0usize, 0u8);
        for k in 0u8..=255 {
            let mut hits = 0usize;
            for s in stems {
                if i == 0 {
                    // key[0] is also used for every position >= KEY_LEN
                    hits += s
                        .iter()
                        .enumerate()
                        .filter(|(j, &b)| (*j == 0 || *j >= KEY_LEN) && in_alphabet(b ^ k))
                        .count();
                } else if let Some(&b) = s.get(i) {
                    hits += in_alphabet(b ^ k) as usize;
                }
            }
            if hits > best.0 {
                best = (hits, k);
            }
        }
        if best.0 == 0 {
            return None;
        }
        *slot = best.1;
    }
    let (mut ok, mut total) = (0usize, 0usize);
    for s in stems {
        for (j, &b) in s.iter().enumerate() {
            total += 1;
            ok += in_alphabet(b ^ names::key_byte(&key, j)) as usize;
        }
    }
    let score = ok as f64 / total as f64;
    Some(NameKeyResult {
        key,
        score,
        names_used: stems.len(),
    })
}

/// Solve the IV from one JJEC file using the known first block.
pub fn recover_iv(key: &[u8; KEY_LEN], jjec: &[u8]) -> Option<[u8; 16]> {
    let enc = Jjec::enc_size(jjec).ok()?;
    if enc < 32 {
        return None;
    }
    let c0: [u8; 16] = jjec[JJEC_HEADER..JJEC_HEADER + 16].try_into().unwrap();
    let d0 = aescbc::decrypt_block(key, &c0);
    let mut iv = [0u8; 16];
    for i in 0..16 {
        iv[i] = d0[i] ^ UNITYFS_BLOCK0[i];
    }
    // Self-check: block 1 must continue the header ("x\0" then the engine version).
    verify(key, &iv, jjec).then_some(iv)
}

/// True if `key`/`iv` turn this JJEC file into a UnityFS bundle whose header continues sanely.
pub fn verify(key: &[u8], iv: &[u8], jjec: &[u8]) -> bool {
    let Ok(enc) = Jjec::enc_size(jjec) else {
        return false;
    };
    if enc < 32 {
        return false;
    }
    let Ok(p) = aescbc::decrypt_raw(key, iv, &jjec[JJEC_HEADER..JJEC_HEADER + 32]) else {
        return false;
    };
    &p[..16] == UNITYFS_BLOCK0 && &p[16..18] == b"x\0" && p[18].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::Jjec;

    const K: &[u8; 16] = b"Synthetic-Key-16";
    const IV: &[u8; 16] = b"Synthetic-IV-016";

    fn synthetic_names() -> Vec<Vec<u8>> {
        let words = [
            "area", "ev", "map", "ui", "common", "statue", "battle", "text", "script", "jp",
            "mind", "effect",
        ];
        let mut v = Vec::new();
        let mut x = 12345u32;
        for _ in 0..600 {
            let mut n = String::new();
            for p in 0..(2 + x % 5) {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                if p > 0 {
                    n.push('_');
                }
                n.push_str(words[(x >> 16) as usize % words.len()]);
                n.push_str(&format!("{}", (x >> 8) % 1000));
            }
            v.push(crate::names::xor(n.as_bytes(), K));
        }
        v
    }

    #[test]
    fn recovers_synthetic_key() {
        let r = recover_name_key(&synthetic_names()).unwrap();
        assert_eq!(&r.key, K);
        assert!(r.score > 0.999, "{}", r.score);
    }

    #[test]
    fn too_few_names() {
        assert_eq!(recover_name_key(&synthetic_names()[..10]), None);
    }

    #[test]
    fn recovers_iv_from_known_header() {
        let mut bundle = UNITYFS_BLOCK0.to_vec();
        bundle.extend_from_slice(b"x\x002018.4.1f1\0");
        bundle.resize(400, 0xAB);
        let c = Jjec::encrypt(&bundle, K, IV, 256).unwrap();
        assert_eq!(recover_iv(K, &c), Some(*IV));
        assert!(verify(K, IV, &c));
        assert!(!verify(K, b"0000000000000000", &c));
    }

    #[test]
    fn iv_rejected_for_non_unity_payload() {
        let c = Jjec::encrypt(&[0x55u8; 300], K, IV, 256).unwrap();
        assert_eq!(recover_iv(K, &c), None);
    }
}
