//! Encrypted file containers written by `ScSystem.ScSecurity.Cryptgraphy` / `ScIO.EncryptFile`.
//!
//! **JJEC** (encrypted asset bundle):
//! `"JJEC" | u32 0 | u32 enc_size | u32 16 | AES-CBC(first enc_size bytes) | plain rest`.
//! Decrypting gives a normal UnityFS bundle.
//!
//! **vcs** (version manifest `vcs.bytes`):
//! `u32 size | u32 enc_size | 32 ASCII md5 | AES-CBC(first enc_size bytes) | plain rest`.

use crate::aescbc::{self, CryptError};

pub const JJEC_MAGIC: &[u8; 4] = b"JJEC";
pub const JJEC_HEADER: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum ContainerError {
    TooShort,
    BadMagic,
    BadHeader(&'static str),
    Crypt(CryptError),
}
impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::TooShort => write!(f, "file too short"),
            ContainerError::BadMagic => write!(f, "bad magic"),
            ContainerError::BadHeader(w) => write!(f, "bad header: {w}"),
            ContainerError::Crypt(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for ContainerError {}
impl From<CryptError> for ContainerError {
    fn from(e: CryptError) -> Self {
        ContainerError::Crypt(e)
    }
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub struct Jjec;

impl Jjec {
    pub fn is_jjec(data: &[u8]) -> bool {
        data.len() >= JJEC_HEADER && &data[..4] == JJEC_MAGIC
    }

    /// Returns the encrypted span length from the header.
    pub fn enc_size(data: &[u8]) -> Result<usize, ContainerError> {
        if data.len() < JJEC_HEADER {
            return Err(ContainerError::TooShort);
        }
        if &data[..4] != JJEC_MAGIC {
            return Err(ContainerError::BadMagic);
        }
        let enc = u32le(data, 8) as usize;
        if u32le(data, 12) != 16 {
            return Err(ContainerError::BadHeader("block size field is not 16"));
        }
        if enc % 16 != 0 || JJEC_HEADER + enc > data.len() {
            return Err(ContainerError::BadHeader("encrypted size out of range"));
        }
        Ok(enc)
    }

    pub fn decrypt(data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>, ContainerError> {
        let enc = Self::enc_size(data)?;
        let mut out = aescbc::decrypt_raw(key, iv, &data[JJEC_HEADER..JJEC_HEADER + enc])?;
        out.extend_from_slice(&data[JJEC_HEADER + enc..]);
        Ok(out)
    }

    /// Inverse of [`Jjec::decrypt`]: encrypt the first `enc_size` bytes (rounded down to 16).
    pub fn encrypt(
        plain: &[u8],
        key: &[u8],
        iv: &[u8],
        max_enc: usize,
    ) -> Result<Vec<u8>, ContainerError> {
        let enc = plain.len().min(max_enc) / 16 * 16;
        let mut out = Vec::with_capacity(plain.len() + JJEC_HEADER);
        out.extend_from_slice(JJEC_MAGIC);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(enc as u32).to_le_bytes());
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&aescbc::encrypt_raw(key, iv, &plain[..enc])?);
        out.extend_from_slice(&plain[enc..]);
        Ok(out)
    }
}

pub struct Vcs;

mod md5 {
    use md5::{Digest, Md5};
    pub fn compute_hex(b: &[u8]) -> String {
        Md5::digest(b).iter().map(|x| format!("{x:02x}")).collect()
    }
}

pub const VCS_HEADER: usize = 40;

impl Vcs {
    /// Returns (declared size, md5 text, plaintext payload).
    pub fn decrypt(
        data: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<(u32, String, Vec<u8>), ContainerError> {
        if data.len() < VCS_HEADER {
            return Err(ContainerError::TooShort);
        }
        let size = u32le(data, 0);
        let enc = u32le(data, 4) as usize;
        let md5_text = std::str::from_utf8(&data[8..40])
            .map_err(|_| ContainerError::BadHeader("md5 not ascii"))?
            .to_string();
        if enc % 16 != 0 || VCS_HEADER + enc > data.len() {
            return Err(ContainerError::BadHeader("encrypted size out of range"));
        }
        if size as usize != data.len() - VCS_HEADER {
            return Err(ContainerError::BadHeader(
                "size field does not match file length",
            ));
        }
        let digest = md5::compute_hex(&data[VCS_HEADER..]);
        if !md5_text.eq_ignore_ascii_case(&digest) {
            return Err(ContainerError::BadHeader(
                "md5 field does not match contents",
            ));
        }
        let mut out = aescbc::decrypt_raw(key, iv, &data[VCS_HEADER..VCS_HEADER + enc])?;
        out.extend_from_slice(&data[VCS_HEADER + enc..]);
        Ok((size, md5_text, out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const K: &[u8; 16] = b"Synthetic-Key-16";
    const IV: &[u8; 16] = b"Synthetic-IV-016";

    #[test]
    fn jjec_round_trip_partial_encryption() {
        let plain: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
        for max in [0, 16, 100, 512, 4096] {
            let c = Jjec::encrypt(&plain, K, IV, max).unwrap();
            assert!(Jjec::is_jjec(&c));
            assert_eq!(c.len(), plain.len() + JJEC_HEADER);
            assert_eq!(Jjec::decrypt(&c, K, IV).unwrap(), plain);
        }
    }

    fn vcs_file(payload: &[u8]) -> Vec<u8> {
        let enc = payload.len() / 16 * 16;
        let mut body = crate::aescbc::encrypt_raw(K, IV, &payload[..enc]).unwrap();
        body.extend_from_slice(&payload[enc..]);
        let mut d = (body.len() as u32).to_le_bytes().to_vec();
        d.extend_from_slice(&(enc as u32).to_le_bytes());
        d.extend_from_slice(md5::compute_hex(&body).as_bytes());
        d.extend_from_slice(&body);
        d
    }

    #[test]
    fn vcs_validates_size_and_md5() {
        let payload = br#"{"keys":["a.bytes"],"values":["0123"]}xyz"#;
        let f = vcs_file(payload);
        assert_eq!(Vcs::decrypt(&f, K, IV).unwrap().2, payload);
        let mut bad = f.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(matches!(
            Vcs::decrypt(&bad, K, IV),
            Err(ContainerError::BadHeader(_))
        ));
        let mut short = f.clone();
        short.pop();
        assert!(matches!(
            Vcs::decrypt(&short, K, IV),
            Err(ContainerError::BadHeader(_))
        ));
    }

    #[test]
    fn jjec_rejects_garbage() {
        assert_eq!(Jjec::enc_size(b"JJEC"), Err(ContainerError::TooShort));
        assert_eq!(Jjec::enc_size(&[0u8; 32]), Err(ContainerError::BadMagic));
        let mut h = b"JJEC\0\0\0\0".to_vec();
        h.extend_from_slice(&1000u32.to_le_bytes());
        h.extend_from_slice(&16u32.to_le_bytes());
        assert!(matches!(
            Jjec::enc_size(&h),
            Err(ContainerError::BadHeader(_))
        ));
    }
}
