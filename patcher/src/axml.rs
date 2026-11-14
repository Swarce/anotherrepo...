//! Binary AndroidManifest.xml (AXML) editing: just enough to add or set an attribute on an element.
//!
//! Layout: `RES_XML` header, a string pool, a resource map (resource ids for the first N strings,
//! which are the attribute names that have framework ids), then XML node chunks whose fields are
//! string-pool indices. To add an attribute with a resource id, its name must be inserted at
//! index N (the end of the mapped range), which shifts every later string index by one, so every
//! string reference in the document is remapped. Attributes are kept sorted by resource id,
//! because the framework's attribute lookup assumes that order.

use std::convert::TryInto;

const RES_XML: u16 = 0x0003;
const STRING_POOL: u16 = 0x0001;
const RES_MAP: u16 = 0x0180;
const NS_START: u16 = 0x0100;
const NS_END: u16 = 0x0101;
const ELEM_START: u16 = 0x0102;
const ELEM_END: u16 = 0x0103;
const CDATA: u16 = 0x0104;
const UTF8_FLAG: u32 = 1 << 8;
const NONE: u32 = 0xFFFF_FFFF;
pub const TYPE_STRING: u8 = 0x03;
pub const TYPE_INT_BOOLEAN: u8 = 0x12;
pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";

pub type Error = String;

fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub ns: u32,
    pub name: u32,
    pub raw: u32,
    pub vtype: u8,
    pub data: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    NsStart {
        line: u32,
        comment: u32,
        prefix: u32,
        uri: u32,
    },
    NsEnd {
        line: u32,
        comment: u32,
        prefix: u32,
        uri: u32,
    },
    Start {
        line: u32,
        comment: u32,
        ns: u32,
        name: u32,
        id: u16,
        class: u16,
        style: u16,
        attrs: Vec<Attr>,
    },
    End {
        line: u32,
        comment: u32,
        ns: u32,
        name: u32,
    },
    Cdata {
        line: u32,
        comment: u32,
        data: u32,
        vtype: u8,
        vdata: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Axml {
    pub strings: Vec<String>,
    pub utf8: bool,
    pub resmap: Vec<u32>,
    pub nodes: Vec<Node>,
}

fn read_pool(b: &[u8]) -> Result<(Vec<String>, bool), Error> {
    let count = u32at(b, 8) as usize;
    let styles = u32at(b, 12);
    let flags = u32at(b, 16);
    let start = u32at(b, 20) as usize;
    if styles != 0 {
        return Err("string pool has styles; not supported".into());
    }
    let utf8 = flags & UTF8_FLAG != 0;
    let mut v = Vec::with_capacity(count);
    for i in 0..count {
        let off = start + u32at(b, 28 + i * 4) as usize;
        if utf8 {
            let mut o = off;
            let skip = |o: &mut usize| {
                let x = b[*o];
                *o += if x & 0x80 != 0 { 2 } else { 1 };
            };
            skip(&mut o); // utf16 length
            let mut n = b[o] as usize;
            if n & 0x80 != 0 {
                n = ((n & 0x7f) << 8) | b[o + 1] as usize;
                o += 2;
            } else {
                o += 1;
            }
            v.push(String::from_utf8(b[o..o + n].to_vec()).map_err(|_| "bad utf8 string")?);
        } else {
            let mut n = u16at(b, off) as usize;
            let mut o = off + 2;
            if n & 0x8000 != 0 {
                n = ((n & 0x7fff) << 16) | u16at(b, off + 2) as usize;
                o += 2;
            }
            let units: Vec<u16> = (0..n).map(|k| u16at(b, o + k * 2)).collect();
            v.push(String::from_utf16(&units).map_err(|_| "bad utf16 string")?);
        }
    }
    Ok((v, utf8))
}

fn write_pool(strings: &[String], utf8: bool) -> Vec<u8> {
    let mut data = Vec::new();
    let mut offs = Vec::new();
    for s in strings {
        offs.push(data.len() as u32);
        if utf8 {
            let u16len = s.encode_utf16().count();
            let b = s.as_bytes();
            for n in [u16len, b.len()] {
                if n > 0x7f {
                    data.push(0x80 | (n >> 8) as u8);
                    data.push(n as u8);
                } else {
                    data.push(n as u8);
                }
            }
            data.extend_from_slice(b);
            data.push(0);
        } else {
            let u: Vec<u16> = s.encode_utf16().collect();
            if u.len() > 0x7fff {
                data.extend_from_slice(&((0x8000 | (u.len() >> 16)) as u16).to_le_bytes());
                data.extend_from_slice(&(u.len() as u16).to_le_bytes());
            } else {
                data.extend_from_slice(&(u.len() as u16).to_le_bytes());
            }
            for c in u {
                data.extend_from_slice(&c.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
        }
    }
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let start = 28 + 4 * strings.len();
    let mut out = Vec::new();
    out.extend_from_slice(&STRING_POOL.to_le_bytes());
    out.extend_from_slice(&28u16.to_le_bytes());
    out.extend_from_slice(&((start + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(if utf8 { UTF8_FLAG } else { 0 }).to_le_bytes());
    out.extend_from_slice(&(start as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for o in offs {
        out.extend_from_slice(&o.to_le_bytes());
    }
    out.extend_from_slice(&data);
    out
}

impl Axml {
    pub fn parse(b: &[u8]) -> Result<Self, Error> {
        if b.len() < 8 || u16at(b, 0) != RES_XML {
            return Err("not a binary XML file".into());
        }
        let total = u32at(b, 4) as usize;
        if total > b.len() {
            return Err("truncated XML".into());
        }
        let mut o = u16at(b, 2) as usize;
        let (mut strings, mut utf8, mut resmap, mut nodes) = (None, false, vec![], vec![]);
        while o + 8 <= total {
            let t = u16at(b, o);
            let sz = u32at(b, o + 4) as usize;
            if sz < 8 || o + sz > total {
                return Err(format!("bad chunk size at {o:#x}"));
            }
            let c = &b[o..o + sz];
            let (line, comment) = if (NS_START..=CDATA).contains(&t) {
                (u32at(c, 8), u32at(c, 12))
            } else {
                (0, 0)
            };
            match t {
                STRING_POOL => {
                    let (s, u) = read_pool(c)?;
                    strings = Some(s);
                    utf8 = u;
                }
                RES_MAP => resmap = (0..(sz - 8) / 4).map(|i| u32at(c, 8 + i * 4)).collect(),
                NS_START | NS_END => {
                    let (prefix, uri) = (u32at(c, 16), u32at(c, 20));
                    nodes.push(if t == NS_START {
                        Node::NsStart {
                            line,
                            comment,
                            prefix,
                            uri,
                        }
                    } else {
                        Node::NsEnd {
                            line,
                            comment,
                            prefix,
                            uri,
                        }
                    });
                }
                ELEM_START => {
                    let (ns, name) = (u32at(c, 16), u32at(c, 20));
                    let (astart, asize, count) = (
                        u16at(c, 24) as usize,
                        u16at(c, 26) as usize,
                        u16at(c, 28) as usize,
                    );
                    if asize != 20 {
                        return Err("unexpected attribute size".into());
                    }
                    let mut attrs = vec![];
                    for i in 0..count {
                        let a = 16 + astart + i * 20;
                        attrs.push(Attr {
                            ns: u32at(c, a),
                            name: u32at(c, a + 4),
                            raw: u32at(c, a + 8),
                            vtype: c[a + 15],
                            data: u32at(c, a + 16),
                        });
                    }
                    nodes.push(Node::Start {
                        line,
                        comment,
                        ns,
                        name,
                        id: u16at(c, 30),
                        class: u16at(c, 32),
                        style: u16at(c, 34),
                        attrs,
                    });
                }
                ELEM_END => nodes.push(Node::End {
                    line,
                    comment,
                    ns: u32at(c, 16),
                    name: u32at(c, 20),
                }),
                CDATA => nodes.push(Node::Cdata {
                    line,
                    comment,
                    data: u32at(c, 16),
                    vtype: c[23],
                    vdata: u32at(c, 24),
                }),
                _ => return Err(format!("unsupported chunk type {t:#x}")),
            }
            o += sz;
        }
        Ok(Self {
            strings: strings.ok_or("no string pool")?,
            utf8,
            resmap,
            nodes,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut body = write_pool(&self.strings, self.utf8);
        if !self.resmap.is_empty() {
            body.extend_from_slice(&RES_MAP.to_le_bytes());
            body.extend_from_slice(&8u16.to_le_bytes());
            body.extend_from_slice(&((8 + 4 * self.resmap.len()) as u32).to_le_bytes());
            for r in &self.resmap {
                body.extend_from_slice(&r.to_le_bytes());
            }
        }
        for n in &self.nodes {
            let mut c: Vec<u8> = Vec::new();
            let head = |t: u16, line: u32, comment: u32, c: &mut Vec<u8>| {
                c.extend_from_slice(&t.to_le_bytes());
                c.extend_from_slice(&16u16.to_le_bytes());
                c.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
                c.extend_from_slice(&line.to_le_bytes());
                c.extend_from_slice(&comment.to_le_bytes());
            };
            let w32 = |v: u32, c: &mut Vec<u8>| c.extend_from_slice(&v.to_le_bytes());
            match n {
                Node::NsStart {
                    line,
                    comment,
                    prefix,
                    uri,
                }
                | Node::NsEnd {
                    line,
                    comment,
                    prefix,
                    uri,
                } => {
                    head(
                        if matches!(n, Node::NsStart { .. }) {
                            NS_START
                        } else {
                            NS_END
                        },
                        *line,
                        *comment,
                        &mut c,
                    );
                    w32(*prefix, &mut c);
                    w32(*uri, &mut c);
                }
                Node::Start {
                    line,
                    comment,
                    ns,
                    name,
                    id,
                    class,
                    style,
                    attrs,
                } => {
                    head(ELEM_START, *line, *comment, &mut c);
                    w32(*ns, &mut c);
                    w32(*name, &mut c);
                    for v in [20u16, 20, attrs.len() as u16, *id, *class, *style] {
                        c.extend_from_slice(&v.to_le_bytes());
                    }
                    for a in attrs {
                        w32(a.ns, &mut c);
                        w32(a.name, &mut c);
                        w32(a.raw, &mut c);
                        c.extend_from_slice(&8u16.to_le_bytes());
                        c.push(0);
                        c.push(a.vtype);
                        w32(a.data, &mut c);
                    }
                }
                Node::End {
                    line,
                    comment,
                    ns,
                    name,
                } => {
                    head(ELEM_END, *line, *comment, &mut c);
                    w32(*ns, &mut c);
                    w32(*name, &mut c);
                }
                Node::Cdata {
                    line,
                    comment,
                    data,
                    vtype,
                    vdata,
                } => {
                    head(CDATA, *line, *comment, &mut c);
                    w32(*data, &mut c);
                    c.extend_from_slice(&8u16.to_le_bytes());
                    c.push(0);
                    c.push(*vtype);
                    w32(*vdata, &mut c);
                }
            }
            let len = c.len() as u32;
            c[4..8].copy_from_slice(&len.to_le_bytes());
            body.extend_from_slice(&c);
        }
        let mut out = Vec::with_capacity(body.len() + 8);
        out.extend_from_slice(&RES_XML.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    pub fn string_index(&self, s: &str) -> Option<u32> {
        self.strings.iter().position(|x| x == s).map(|i| i as u32)
    }

    fn resid_of(&self, name: u32) -> Option<u32> {
        self.resmap.get(name as usize).copied().filter(|&r| r != 0)
    }

    /// Insert a string at `at`, shifting every reference >= `at`.
    fn insert_string(&mut self, at: usize, s: &str) {
        self.strings.insert(at, s.to_owned());
        let at = at as u32;
        let fix = |r: &mut u32| {
            if *r != NONE && *r >= at {
                *r += 1;
            }
        };
        for n in &mut self.nodes {
            match n {
                Node::NsStart {
                    comment,
                    prefix,
                    uri,
                    ..
                }
                | Node::NsEnd {
                    comment,
                    prefix,
                    uri,
                    ..
                } => {
                    fix(comment);
                    fix(prefix);
                    fix(uri);
                }
                Node::Start {
                    comment,
                    ns,
                    name,
                    attrs,
                    ..
                } => {
                    fix(comment);
                    fix(ns);
                    fix(name);
                    for a in attrs {
                        fix(&mut a.ns);
                        fix(&mut a.name);
                        fix(&mut a.raw);
                        if a.vtype == TYPE_STRING {
                            fix(&mut a.data);
                        }
                    }
                }
                Node::End {
                    comment, ns, name, ..
                } => {
                    fix(comment);
                    fix(ns);
                    fix(name);
                }
                Node::Cdata {
                    comment,
                    data,
                    vtype,
                    vdata,
                    ..
                } => {
                    fix(comment);
                    fix(data);
                    if *vtype == TYPE_STRING {
                        fix(vdata);
                    }
                }
            }
        }
    }

    /// Set `android:<attr_name>` (framework id `resid`) on the first `<element>` to a boolean.
    /// Returns "added" or "updated".
    pub fn set_android_bool(
        &mut self,
        element: &str,
        attr_name: &str,
        resid: u32,
        value: bool,
    ) -> Result<&'static str, Error> {
        let el = self
            .string_index(element)
            .ok_or_else(|| format!("no <{element}> element"))?;
        let data = if value { NONE } else { 0 };
        // existing attribute (matched by resource id)?
        let found = self
            .nodes
            .iter()
            .position(|n| matches!(n, Node::Start { name, .. } if *name == el))
            .ok_or_else(|| format!("no <{element}> element"))?;
        let existing = match &self.nodes[found] {
            Node::Start { attrs, .. } => attrs
                .iter()
                .position(|a| self.resid_of(a.name) == Some(resid)),
            _ => None,
        };
        if let Some(i) = existing {
            if let Node::Start { attrs, .. } = &mut self.nodes[found] {
                attrs[i].vtype = TYPE_INT_BOOLEAN;
                attrs[i].data = data;
                attrs[i].raw = NONE;
            }
            return Ok("updated");
        }
        let ns = self
            .string_index(ANDROID_NS)
            .ok_or("no android namespace string")?;
        let at = self.resmap.len();
        self.insert_string(at, attr_name);
        self.resmap.push(resid);
        let name_idx = at as u32;
        let ns = if ns >= name_idx { ns + 1 } else { ns };
        let resmap = self.resmap.clone();
        let resid_of = |n: u32| resmap.get(n as usize).copied().filter(|&r| r != 0);
        if let Node::Start {
            attrs,
            id,
            class,
            style,
            ..
        } = &mut self.nodes[found]
        {
            let pos = attrs
                .iter()
                .position(|a| resid_of(a.name).map(|r| r > resid).unwrap_or(true))
                .unwrap_or(attrs.len());
            attrs.insert(
                pos,
                Attr {
                    ns,
                    name: name_idx,
                    raw: NONE,
                    vtype: TYPE_INT_BOOLEAN,
                    data,
                },
            );
            for ix in [id, class, style] {
                if *ix as usize > pos {
                    *ix += 1;
                }
            }
        }
        Ok("added")
    }

    /// Boolean value of `android:<resid>` on the first `<element>`, if present.
    pub fn get_android_bool(&self, element: &str, resid: u32) -> Option<bool> {
        let el = self.string_index(element)?;
        self.nodes.iter().find_map(|n| match n {
            Node::Start { name, attrs, .. } if *name == el => attrs
                .iter()
                .find(|a| self.resid_of(a.name) == Some(resid))
                .map(|a| a.vtype == TYPE_INT_BOOLEAN && a.data != 0),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic manifest: <manifest xmlns:android=...><application android:label="x" android:debuggable="false"/></manifest>
    fn synth(utf8: bool) -> Axml {
        // strings 0..1 are mapped attribute names
        let strings = vec![
            "label",
            "debuggable",
            "android",
            ANDROID_NS,
            "manifest",
            "application",
            "x",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let a = |name, vtype, data, raw| Attr {
            ns: 3,
            name,
            raw,
            vtype,
            data,
        };
        Axml {
            strings,
            utf8,
            resmap: vec![0x01010001, 0x0101000f],
            nodes: vec![
                Node::NsStart {
                    line: 1,
                    comment: NONE,
                    prefix: 2,
                    uri: 3,
                },
                Node::Start {
                    line: 1,
                    comment: NONE,
                    ns: NONE,
                    name: 4,
                    id: 0,
                    class: 0,
                    style: 0,
                    attrs: vec![],
                },
                Node::Start {
                    line: 2,
                    comment: NONE,
                    ns: NONE,
                    name: 5,
                    id: 0,
                    class: 0,
                    style: 2,
                    attrs: vec![a(0, TYPE_STRING, 6, 6), a(1, TYPE_INT_BOOLEAN, 0, NONE)],
                },
                Node::End {
                    line: 2,
                    comment: NONE,
                    ns: NONE,
                    name: 5,
                },
                Node::End {
                    line: 3,
                    comment: NONE,
                    ns: NONE,
                    name: 4,
                },
                Node::NsEnd {
                    line: 3,
                    comment: NONE,
                    prefix: 2,
                    uri: 3,
                },
            ],
        }
    }

    #[test]
    fn write_parse_round_trip() {
        for utf8 in [false, true] {
            let x = synth(utf8);
            assert_eq!(Axml::parse(&x.write()).unwrap(), x);
        }
    }

    #[test]
    fn add_attribute_remaps_and_sorts() {
        let mut x = synth(false);
        assert_eq!(
            x.set_android_bool("application", "usesCleartextTraffic", 0x010104ec, true)
                .unwrap(),
            "added"
        );
        let y = Axml::parse(&x.write()).unwrap();
        assert_eq!(y.strings[2], "usesCleartextTraffic");
        assert_eq!(y.resmap, vec![0x01010001, 0x0101000f, 0x010104ec]);
        assert_eq!(y.get_android_bool("application", 0x010104ec), Some(true));
        if let Node::Start { attrs, style, .. } = &y.nodes[2] {
            // label (string value "x" moved from 6 to 7), debuggable, then the new one (highest id)
            assert_eq!(attrs.len(), 3);
            assert_eq!((attrs[0].data, attrs[0].raw), (7, 7));
            assert_eq!(y.strings[attrs[0].data as usize], "x");
            assert_eq!(attrs[2].name, 2);
            assert_eq!(attrs[2].ns, 4);
            assert_eq!(y.strings[attrs[2].ns as usize], ANDROID_NS);
            assert_eq!(
                *style, 2,
                "style index points before insertion point, unchanged"
            );
        } else {
            panic!()
        }
        if let Node::NsStart { uri, .. } = y.nodes[0] {
            assert_eq!(y.strings[uri as usize], ANDROID_NS);
        }
        // idempotent: second call updates
        let mut z = y.clone();
        assert_eq!(
            z.set_android_bool("application", "usesCleartextTraffic", 0x010104ec, true)
                .unwrap(),
            "updated"
        );
        assert_eq!(z.strings.len(), y.strings.len());
    }

    #[test]
    fn insert_in_middle_shifts_style_index() {
        let mut x = synth(false);
        // id lower than both existing attributes -> inserted at position 0
        x.set_android_bool("application", "aaa", 0x01010000, true)
            .unwrap();
        if let Node::Start { attrs, style, .. } = &x.nodes[2] {
            assert_eq!(x.strings[attrs[0].name as usize], "aaa");
            assert_eq!(*style, 3);
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(Axml::parse(b"nope").is_err());
        assert!(Axml::parse(&[3, 0, 8, 0, 255, 0, 0, 0]).is_err());
    }
}
