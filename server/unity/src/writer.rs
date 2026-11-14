//! Little-endian byte writer with Unity's alignment conventions.

#[derive(Default, Clone)]
pub struct W {
    pub buf: Vec<u8>,
}

impl W {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32be(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i64be(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    pub fn cstr(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
    /// Pad with zeros to a multiple of `n`.
    pub fn align(&mut self, n: usize) {
        while self.buf.len() % n != 0 {
            self.buf.push(0);
        }
    }
    /// Unity `string`: i32 length, bytes, then align to 4.
    pub fn string(&mut self, s: &str) {
        self.i32(s.len() as i32);
        self.bytes(s.as_bytes());
        self.align(4);
    }
    /// `bool` with the align flag: one byte, then align to 4.
    pub fn bool_aligned(&mut self, v: bool) {
        self.u8(v as u8);
        self.align(4);
    }
    /// `PPtr<T>`: i32 file id + i64 path id.
    pub fn pptr(&mut self, file_id: i32, path_id: i64) {
        self.i32(file_id);
        self.i64(path_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn string_alignment() {
        let mut w = W::new();
        w.string("abc");
        assert_eq!(w.buf, [3, 0, 0, 0, b'a', b'b', b'c', 0]);
        w.string("");
        assert_eq!(w.len(), 12);
        w.bool_aligned(true);
        assert_eq!(w.len(), 16);
    }
}
