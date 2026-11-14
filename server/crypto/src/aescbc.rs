//! AES-128-CBC helpers (the client's `RijndaelManaged` with 128-bit blocks).

use aes::cipher::{
    block_padding::NoPadding, block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit,
};

type Enc = cbc::Encryptor<aes::Aes128>;
type Dec = cbc::Decryptor<aes::Aes128>;
type Enc256 = cbc::Encryptor<aes::Aes256>;
type Dec256 = cbc::Decryptor<aes::Aes256>;

#[derive(Debug, PartialEq, Eq)]
pub enum CryptError {
    BadKeyLength(usize),
    BadIvLength(usize),
    NotBlockAligned(usize),
    BadPadding,
}

impl std::fmt::Display for CryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptError::BadKeyLength(n) => write!(f, "key must be 16 or 32 bytes, got {n}"),
            CryptError::BadIvLength(n) => write!(f, "IV must be 16 bytes, got {n}"),
            CryptError::NotBlockAligned(n) => {
                write!(f, "ciphertext length {n} is not a multiple of 16")
            }
            CryptError::BadPadding => write!(f, "bad PKCS7 padding (wrong key or IV?)"),
        }
    }
}
impl std::error::Error for CryptError {}

fn check(key: &[u8], iv: &[u8]) -> Result<(), CryptError> {
    if key.len() != 16 && key.len() != 32 {
        return Err(CryptError::BadKeyLength(key.len()));
    }
    if iv.len() != 16 {
        return Err(CryptError::BadIvLength(iv.len()));
    }
    Ok(())
}

/// Encrypt with PKCS7 padding. Key length selects AES-128 or AES-256.
pub fn encrypt_pkcs7(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptError> {
    check(key, iv)?;
    Ok(if key.len() == 16 {
        Enc::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(data)
    } else {
        Enc256::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(data)
    })
}

/// Decrypt and strip PKCS7 padding.
pub fn decrypt_pkcs7(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptError> {
    check(key, iv)?;
    if data.len() % 16 != 0 {
        return Err(CryptError::NotBlockAligned(data.len()));
    }
    let r = if key.len() == 16 {
        Dec::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(data)
    } else {
        Dec256::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(data)
    };
    r.map_err(|_| CryptError::BadPadding)
}

/// Decrypt without touching padding (container formats carry their own sizes).
pub fn decrypt_raw(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptError> {
    check(key, iv)?;
    if data.len() % 16 != 0 {
        return Err(CryptError::NotBlockAligned(data.len()));
    }
    Ok(if key.len() == 16 {
        Dec::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<NoPadding>(data)
            .unwrap()
    } else {
        Dec256::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<NoPadding>(data)
            .unwrap()
    })
}

/// Encrypt block-aligned data without padding.
pub fn encrypt_raw(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptError> {
    check(key, iv)?;
    if data.len() % 16 != 0 {
        return Err(CryptError::NotBlockAligned(data.len()));
    }
    Ok(if key.len() == 16 {
        Enc::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<NoPadding>(data)
    } else {
        Enc256::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<NoPadding>(data)
    })
}

/// Decrypt a single block in ECB mode (used to solve an unknown IV from known plaintext).
pub fn decrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
    use aes::cipher::{BlockDecrypt, KeyInit};
    let c = aes::Aes128::new(key.into());
    let mut b = aes::Block::clone_from_slice(block);
    c.decrypt_block(&mut b);
    b.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    const K: &[u8; 16] = b"0123456789abcdef";
    const IV: &[u8; 16] = b"fedcba9876543210";

    #[test]
    fn pkcs7_round_trip_all_lengths() {
        for n in 0..70 {
            let data: Vec<u8> = (0..n as u8).collect();
            let c = encrypt_pkcs7(K, IV, &data).unwrap();
            assert_eq!(c.len() % 16, 0);
            assert_eq!(decrypt_pkcs7(K, IV, &c).unwrap(), data);
        }
    }

    #[test]
    fn aes256_round_trip() {
        let k = [7u8; 32];
        let c = encrypt_pkcs7(&k, IV, b"hello world").unwrap();
        assert_eq!(decrypt_pkcs7(&k, IV, &c).unwrap(), b"hello world");
    }

    #[test]
    fn wrong_key_is_an_error_not_a_panic() {
        let c = encrypt_pkcs7(K, IV, b"some plaintext here").unwrap();
        let r = decrypt_pkcs7(b"XXXXXXXXXXXXXXXX", IV, &c);
        // Usually BadPadding; a 1/256 chance of a valid-looking pad is fine, it just must not panic.
        let _ = r;
        assert_eq!(
            decrypt_pkcs7(K, IV, &c[..15]),
            Err(CryptError::NotBlockAligned(15))
        );
        assert_eq!(
            encrypt_pkcs7(&K[..5], IV, b""),
            Err(CryptError::BadKeyLength(5))
        );
    }

    #[test]
    fn raw_and_block_agree() {
        let pt = [42u8; 32];
        let c = encrypt_raw(K, IV, &pt).unwrap();
        assert_eq!(decrypt_raw(K, IV, &c).unwrap(), pt);
        // first block: D(C0) xor IV == P0
        let d0 = decrypt_block(K, c[..16].try_into().unwrap());
        let p0: Vec<u8> = d0.iter().zip(IV).map(|(a, b)| a ^ b).collect();
        assert_eq!(p0, &pt[..16]);
    }
}
