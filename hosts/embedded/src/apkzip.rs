//! Read one STORED entry from the app's own APK (no AssetManager/JNI needed). The patcher writes
//! `assets/jdr/keys.json` uncompressed for this.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Path of the APK this process runs from, from `/proc/self/maps`.
pub fn own_apk_path() -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    maps.lines()
        .filter_map(|l| l.split_whitespace().nth(5))
        .find(|p| p.ends_with("/base.apk"))
        .map(str::to_owned)
}

pub fn read_stored(apk: &str, name: &str) -> Result<Vec<u8>, String> {
    let mut f = File::open(apk).map_err(|e| format!("{apk}: {e}"))?;
    let len = f.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let tail_len = len.min(66_000);
    f.seek(SeekFrom::Start(len - tail_len))
        .map_err(|e| e.to_string())?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail).map_err(|e| e.to_string())?;
    let eocd = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| u32le(&tail, i) == 0x0605_4b50)
        .ok_or("no end of central directory")?;
    let count = u16le(&tail, eocd + 10) as usize;
    let cd_size = u32le(&tail, eocd + 12) as usize;
    let cd_off = u32le(&tail, eocd + 16) as u64;
    if cd_off == 0xFFFF_FFFF || cd_size > 64 << 20 {
        return Err("zip64 or oversized central directory not supported".into());
    }
    f.seek(SeekFrom::Start(cd_off)).map_err(|e| e.to_string())?;
    let mut cd = vec![0u8; cd_size];
    f.read_exact(&mut cd).map_err(|e| e.to_string())?;
    let mut o = 0;
    for _ in 0..count {
        if o + 46 > cd.len() || u32le(&cd, o) != 0x0201_4b50 {
            return Err("bad central directory".into());
        }
        let method = u16le(&cd, o + 10);
        let csize = u32le(&cd, o + 20) as usize;
        let nlen = u16le(&cd, o + 28) as usize;
        let elen = u16le(&cd, o + 30) as usize;
        let clen = u16le(&cd, o + 32) as usize;
        let lho = u32le(&cd, o + 42) as u64;
        let n = &cd[o + 46..o + 46 + nlen];
        if n == name.as_bytes() {
            if method != 0 {
                return Err(format!("{name} is compressed; expected stored"));
            }
            let mut lh = [0u8; 30];
            f.seek(SeekFrom::Start(lho)).map_err(|e| e.to_string())?;
            f.read_exact(&mut lh).map_err(|e| e.to_string())?;
            if u32le(&lh, 0) != 0x0403_4b50 {
                return Err("bad local header".into());
            }
            let data_off = lho + 30 + u16le(&lh, 26) as u64 + u16le(&lh, 28) as u64;
            f.seek(SeekFrom::Start(data_off))
                .map_err(|e| e.to_string())?;
            let mut out = vec![0u8; csize];
            f.read_exact(&mut out).map_err(|e| e.to_string())?;
            return Ok(out);
        }
        o += 46 + nlen + elen + clen;
    }
    Err(format!("{name} not found in {apk}"))
}
