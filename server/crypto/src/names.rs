//! Bundle file name obfuscation (`ResourceManager.ConvObfuscationName`, `XorCrypt`).
//!
//! On-disk name = lowercase hex of `xor(utf8(name), key)` + original extension, where byte `i`
//! is XORed with `key[i]` while `i < key.len()` and with `key[0]` after that
//! (`XorCrypt.xorEnc` @0x1060F80 uses `csinc` to clamp the key index).

/// Key byte used for position `i`.
#[inline]
pub fn key_byte(key: &[u8], i: usize) -> u8 {
    if i < key.len() {
        key[i]
    } else {
        key[0]
    }
}

pub fn xor(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ key_byte(key, i))
        .collect()
}

/// `"ui_common"` -> obfuscated stem (no extension).
pub fn obfuscate(name: &str, key: &[u8]) -> String {
    hex(&xor(name.as_bytes(), key))
}

/// Obfuscated stem -> original name. `None` if not hex or not UTF-8.
pub fn deobfuscate(stem: &str, key: &[u8]) -> Option<String> {
    let raw = unhex(stem)?;
    String::from_utf8(xor(&raw, key)).ok()
}

pub fn hex(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char);
    }
    s
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || s.is_empty() {
        return None;
    }
    let v = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    s.as_bytes()
        .chunks(2)
        .map(|p| Some(v(p[0])? << 4 | v(p[1])?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const K: &[u8] = b"Synthetic-Key-16";

    #[test]
    fn round_trip_short_and_long() {
        for n in [
            "a",
            "ui_common",
            "e_ev_eq_500001_abcdefgh_04_script_text_jp",
            "text.jp",
        ] {
            let o = obfuscate(n, K);
            assert_eq!(deobfuscate(&o, K).as_deref(), Some(n));
        }
    }

    #[test]
    fn positions_past_key_use_first_byte() {
        let name = "x".repeat(20);
        let raw = unhex(&obfuscate(&name, K)).unwrap();
        for (i, b) in raw.iter().enumerate().skip(16) {
            assert_eq!(b ^ b'x', K[0], "pos {i}");
        }
    }

    #[test]
    fn bad_hex() {
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
        assert_eq!(unhex(""), None);
    }
}
