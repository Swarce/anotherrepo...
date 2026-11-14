//! UnityFS bundles (format 6, Unity 2018.4).
//!
//! Layout: `"UnityFS\0"`, u32 format, `"5.x.x\0"`, engine version `\0`, i64 total size,
//! u32 compressed/uncompressed blocks-info size, u32 flags (low 6 bits = compression of the
//! blocks info; 0x40 = directory present; 0x80 = blocks info at the end), then the blocks info
//! (16-byte hash, block table, node table) and the data blocks. All header integers big-endian.

use crate::writer::W;
use crate::Error;

const MAGIC: &[u8] = b"UnityFS\0";

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    pub data: Vec<u8>,
    pub flags: u32,
}

struct R<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let s = self.b.get(self.o..self.o + n).ok_or("bundle truncated")?;
        self.o += n;
        Ok(s)
    }
    fn u16be(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32be(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64be(&mut self) -> Result<i64, Error> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn cstr(&mut self) -> Result<String, Error> {
        let rest = &self.b[self.o.min(self.b.len())..];
        let end = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or("unterminated string")?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.o += end + 1;
        Ok(s)
    }
}

fn decompress(kind: u32, src: &[u8], out_len: usize) -> Result<Vec<u8>, Error> {
    match kind {
        0 => Ok(src.to_vec()),
        2 | 3 => {
            let v = lz4_flex::block::decompress(src, out_len).map_err(|e| format!("lz4: {e}"))?;
            if v.len() != out_len {
                return Err("lz4: wrong decompressed size".into());
            }
            Ok(v)
        }
        k => Err(format!(
            "unsupported compression {k} (LZMA bundles are not used by this game)"
        )),
    }
}

/// Read all nodes (files) of a UnityFS bundle.
pub fn read(b: &[u8]) -> Result<Vec<Node>, Error> {
    let mut r = R { b, o: 0 };
    if r.take(8)? != MAGIC {
        return Err("not a UnityFS bundle".into());
    }
    let format = r.u32be()?;
    if format != 6 && format != 7 {
        return Err(format!("unsupported UnityFS format {format}"));
    }
    let _player = r.cstr()?;
    let _engine = r.cstr()?;
    let _size = r.i64be()?;
    let csize = r.u32be()? as usize;
    let usize_ = r.u32be()? as usize;
    let flags = r.u32be()?;
    if format >= 7 {
        r.o = r.o.div_ceil(16) * 16;
    }
    let info_c = if flags & 0x80 != 0 {
        b.get(b.len().saturating_sub(csize)..)
            .ok_or("bad blocks info")?
    } else {
        r.take(csize)?
    };
    let info = decompress(flags & 0x3f, info_c, usize_)?;
    let mut ir = R { b: &info, o: 16 };
    let nblocks = ir.u32be()? as usize;
    let mut blocks = Vec::with_capacity(nblocks);
    for _ in 0..nblocks {
        let us = ir.u32be()? as usize;
        let cs = ir.u32be()? as usize;
        let fl = ir.u16be()? as u32;
        blocks.push((us, cs, fl));
    }
    let nnodes = ir.u32be()? as usize;
    let mut nodes_meta = Vec::with_capacity(nnodes);
    for _ in 0..nnodes {
        let off = ir.i64be()? as usize;
        let sz = ir.i64be()? as usize;
        let fl = ir.u32be()?;
        let name = ir.cstr()?;
        nodes_meta.push((off, sz, fl, name));
    }
    let mut data = Vec::new();
    for (us, cs, fl) in blocks {
        data.extend_from_slice(&decompress(fl & 0x3f, r.take(cs)?, us)?);
    }
    nodes_meta
        .into_iter()
        .map(|(off, sz, fl, name)| {
            Ok(Node {
                name,
                data: data.get(off..off + sz).ok_or("node out of range")?.to_vec(),
                flags: fl,
            })
        })
        .collect()
}

/// Write an uncompressed UnityFS bundle (one data block per node, max 128 KiB? no: Unity accepts
/// a single uncompressed block of any size).
pub fn write(nodes: &[Node], engine: &str) -> Vec<u8> {
    let mut data = Vec::new();
    let mut table = Vec::new();
    for n in nodes {
        table.push((
            data.len() as i64,
            n.data.len() as i64,
            n.flags,
            n.name.clone(),
        ));
        data.extend_from_slice(&n.data);
    }
    let mut info = W::new();
    info.bytes(&[0u8; 16]);
    info.u32be(1);
    info.u32be(data.len() as u32);
    info.u32be(data.len() as u32);
    info.buf.extend_from_slice(&0u16.to_be_bytes()); // block flags: uncompressed
    info.u32be(table.len() as u32);
    for (off, sz, fl, name) in &table {
        info.i64be(*off);
        info.i64be(*sz);
        info.u32be(*fl);
        info.cstr(name);
    }
    let mut w = W::new();
    w.bytes(MAGIC);
    w.u32be(6);
    w.cstr("5.x.x");
    w.cstr(engine);
    let header_len = w.len() + 8 + 4 + 4 + 4;
    let total = header_len + info.len() + data.len();
    w.i64be(total as i64);
    w.u32be(info.len() as u32);
    w.u32be(info.len() as u32);
    w.u32be(0x40); // uncompressed blocks info, directory present
    w.bytes(&info.buf);
    w.bytes(&data);
    w.buf
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let nodes = vec![
            Node {
                name: "CAB-0123".into(),
                data: (0..5000u32).map(|i| i as u8).collect(),
                flags: 4,
            },
            Node {
                name: "CAB-0123.resS".into(),
                data: vec![9; 10],
                flags: 0,
            },
        ];
        let b = write(&nodes, "2018.4.1f1");
        assert_eq!(read(&b).unwrap(), nodes);
        assert!(read(b"nope").is_err());
        assert!(read(&b[..40]).is_err());
    }
}
