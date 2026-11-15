//! Japanese text built into the APK itself, outside any table or bundle:
//!
//! - `UnityEngine.UI.Text` labels in the scenes built into the APK (`assets/bin/Data/level*`: the
//!   loading overlay, the download, save-select, contact and error screens). Keys
//!   `apk:<file>:<path id>:m_Text`.
//! - player-visible C# string literals in `global-metadata.dat` (button labels, rank and date
//!   formats, …), chosen by an allow-list per verified build (`known::VerifiedBuild::ui_literals`).
//!   Keys `literal:<index>`.
//!
//! The scene files carry no type trees. A `UI.Text` (Unity 2018.4) is decoded here field by field
//! and must consume exactly its bytes; `m_Text` is its last field, so a new text changes nothing
//! before it.

use jdr_unity::sfile;
use jdr_unity::writer::W;

pub type Error = String;

pub const CLASS_MONOBEHAVIOUR: i32 = 114;
/// `m_OnCullStateChanged.m_TypeName` of every `MaskableGraphic` (Text, Image, …).
const CULL_EVENT_TYPE: &str =
    "UnityEngine.UI.MaskableGraphic+CullStateChangedEvent, UnityEngine.UI";

/// Directory of the APK's Unity data.
pub const DATA_DIR: &str = "assets/bin/Data/";

/// `assets/bin/Data/level<N>`: a scene built into the APK.
pub fn is_scene(entry: &str) -> bool {
    entry
        .strip_prefix(DATA_DIR)
        .and_then(|f| f.strip_prefix("level"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
}

pub fn scene_key(file: &str, path_id: i64) -> String {
    format!("apk:{file}:{path_id}:m_Text")
}

pub fn literal_key(index: usize) -> String {
    format!("literal:{index}")
}

/// A decoded `UI.Text`: where its `m_Text` starts and the text.
#[derive(Debug, Clone, PartialEq)]
pub struct UiText {
    pub text_offset: usize,
    pub text: String,
}

struct R<'a> {
    b: &'a [u8],
    o: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let end = self.o.checked_add(n).ok_or("overflow")?;
        let s = self.b.get(self.o..end).ok_or("UI.Text truncated")?;
        self.o = end;
        Ok(s)
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn align(&mut self) {
        self.o = self.o.div_ceil(4) * 4;
    }
    fn pptr(&mut self) -> Result<(), Error> {
        self.take(12).map(|_| ())
    }
    fn bool(&mut self) -> Result<(), Error> {
        let v = self.take(1)?[0];
        if v > 1 {
            return Err("UI.Text: bad bool".into());
        }
        self.align();
        Ok(())
    }
    fn string(&mut self) -> Result<String, Error> {
        let n = self.i32()?;
        if !(0..=1 << 20).contains(&n) {
            return Err("UI.Text: bad string length".into());
        }
        let s = self.take(n as usize)?.to_vec();
        self.align();
        String::from_utf8(s).map_err(|_| "UI.Text: invalid UTF-8".into())
    }
}

/// Decode a `UI.Text` MonoBehaviour (Unity 2018.4 layout). Anything else is an error.
pub fn parse_ui_text(d: &[u8]) -> Result<UiText, Error> {
    let mut r = R { b: d, o: 0 };
    r.pptr()?; // m_GameObject
    r.bool()?; // m_Enabled
    r.pptr()?; // m_Script
    r.string()?; // m_Name
    r.pptr()?; // m_Material
    r.take(16)?; // m_Color
    r.bool()?; // m_RaycastTarget
               // m_OnCullStateChanged.m_PersistentCalls.m_Calls
    let calls = r.i32()?;
    if !(0..=64).contains(&calls) {
        return Err("UI.Text: bad call count".into());
    }
    for _ in 0..calls {
        r.pptr()?; // m_Target
        r.string()?; // m_MethodName
        r.i32()?; // m_Mode
        r.pptr()?; // m_Arguments.m_ObjectArgument
        r.string()?; // m_ObjectArgumentAssemblyTypeName
        r.i32()?; // m_IntArgument
        r.i32()?; // m_FloatArgument
        r.string()?; // m_StringArgument
        r.bool()?; // m_BoolArgument
        r.i32()?; // m_CallState
    }
    if !r.string()?.starts_with(CULL_EVENT_TYPE) {
        return Err("not a MaskableGraphic".into());
    }
    // m_FontData
    r.pptr()?; // m_Font
    r.i32()?; // m_FontSize
    r.i32()?; // m_FontStyle
    r.bool()?; // m_BestFit
    r.i32()?; // m_MinSize
    r.i32()?; // m_MaxSize
    r.i32()?; // m_Alignment
    r.bool()?; // m_AlignByGeometry
    r.bool()?; // m_RichText
    r.i32()?; // m_HorizontalOverflow
    r.i32()?; // m_VerticalOverflow
    r.i32()?; // m_LineSpacing
    let text_offset = r.o;
    let text = r.string()?;
    if r.o != d.len() {
        return Err(format!(
            "UI.Text: {} bytes after m_Text",
            d.len() as isize - r.o as isize
        ));
    }
    Ok(UiText { text_offset, text })
}

/// The same object with another `m_Text`.
pub fn set_ui_text(d: &[u8], text: &str) -> Result<Vec<u8>, Error> {
    let t = parse_ui_text(d)?;
    let mut w = W::new();
    w.bytes(&d[..t.text_offset]);
    w.string(text);
    Ok(w.buf)
}

/// Every `UI.Text` of a scene file: (path id, text).
pub fn scene_texts(file: &[u8]) -> Result<Vec<(i64, String)>, Error> {
    let f = sfile::parse(file)?;
    Ok(f.objects
        .iter()
        .filter(|o| {
            f.types
                .get(o.type_index as usize)
                .is_some_and(|t| t.entry.class_id == CLASS_MONOBEHAVIOUR)
        })
        .filter_map(|o| Some((o.path_id, parse_ui_text(&o.data).ok()?.text)))
        .collect())
}

/// The Japanese labels of scene `file` (e.g. `level2`) as (key, source), for the export.
pub fn scene_sources(file: &str, data: &[u8]) -> Result<Vec<(String, String)>, Error> {
    Ok(scene_texts(data)?
        .into_iter()
        .filter(|(_, t)| jdr_services::l10n::has_japanese(t))
        .map(|(pid, t)| (scene_key(file, pid), t))
        .collect())
}

/// The allow-listed literals of `build` as (key, source), for the export.
pub fn literal_sources(
    metadata: &[u8],
    build: &crate::known::VerifiedBuild,
) -> Result<Vec<(String, String)>, Error> {
    let m = crate::meta::Metadata::parse(metadata).map_err(|e| e.to_string())?;
    build
        .ui_literals
        .iter()
        .map(|(i, _)| {
            let b = m.literal(*i).map_err(|e| e.to_string())?;
            let s = String::from_utf8(b.to_vec())
                .map_err(|_| format!("metadata literal {i} is not UTF-8"))?;
            Ok((literal_key(*i), s))
        })
        .collect()
}

/// Replace `m_Text` of the `UI.Text` objects for which `english(path id, text)` returns a string.
/// Returns (file, replaced). The output is decoded again and checked: the edited objects hold
/// the new text with the bytes before it unchanged, every other object and table is unchanged.
pub fn rewrite_scene(
    file: &[u8],
    english: &dyn Fn(i64, &str) -> Option<String>,
) -> Result<(Vec<u8>, usize), Error> {
    let mut f = sfile::parse(file)?;
    if f.write() != file {
        return Err("scene file does not round-trip unchanged; refusing".into());
    }
    let orig = f.objects.clone();
    let mut want: Vec<Option<String>> = vec![None; orig.len()];
    for (i, o) in f.objects.iter_mut().enumerate() {
        if f.types.get(o.type_index as usize).map(|t| t.entry.class_id) != Some(CLASS_MONOBEHAVIOUR)
        {
            continue;
        }
        let Ok(t) = parse_ui_text(&o.data) else {
            continue;
        };
        if let Some(en) = english(o.path_id, &t.text) {
            o.data = set_ui_text(&o.data, &en)?;
            want[i] = Some(en);
        }
    }
    let n = want.iter().flatten().count();
    let out = if n == 0 { file.to_vec() } else { f.write() };
    // Decode the output and compare it with the input, object by object.
    let g = sfile::parse(&out)?;
    if g.objects.len() != orig.len() || g.types.len() != f.types.len() {
        return Err("rewritten scene: object or type count changed".into());
    }
    for ((a, b), w) in orig.iter().zip(&g.objects).zip(&want) {
        if a.path_id != b.path_id || a.type_index != b.type_index {
            return Err(format!("rewritten scene: object {} moved", a.path_id));
        }
        match w {
            None if a.data != b.data => {
                return Err(format!("rewritten scene: object {} changed", a.path_id))
            }
            None => {}
            Some(en) => {
                let (ta, tb) = (parse_ui_text(&a.data)?, parse_ui_text(&b.data)?);
                if tb.text != *en
                    || ta.text_offset != tb.text_offset
                    || a.data[..ta.text_offset] != b.data[..tb.text_offset]
                {
                    return Err(format!("rewritten scene: object {} is wrong", a.path_id));
                }
            }
        }
    }
    Ok((out, n))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic `UI.Text` with the given number of persistent calls.
    pub fn ui_text(text: &str, calls: i32) -> Vec<u8> {
        let mut w = W::new();
        w.pptr(0, 5);
        w.bool_aligned(true);
        w.pptr(1, 2591);
        w.string("");
        w.pptr(0, 0);
        for _ in 0..4 {
            w.u32(1.0f32.to_bits());
        }
        w.bool_aligned(false);
        w.i32(calls);
        for _ in 0..calls {
            w.pptr(0, 9);
            w.string("OnCull");
            w.i32(1);
            w.pptr(0, 0);
            w.string("UnityEngine.Object, UnityEngine");
            w.i32(0);
            w.i32(0);
            w.string("");
            w.bool_aligned(false);
            w.i32(2);
        }
        w.string(&format!(
            "{CULL_EVENT_TYPE}, Version=1.0.0.0, Culture=neutral, PublicKeyToken=null"
        ));
        w.pptr(10, 3);
        for v in [20, 0] {
            w.i32(v);
        }
        w.bool_aligned(false);
        for v in [0, 40, 4] {
            w.i32(v);
        }
        w.bool_aligned(false);
        w.bool_aligned(true);
        for v in [0, 0] {
            w.i32(v);
        }
        w.u32(1.0f32.to_bits());
        w.string(text);
        w.buf
    }

    #[test]
    fn ui_text_decodes_and_rewrites() {
        for calls in [0, 2] {
            let d = ui_text("テスト文字列", calls);
            let t = parse_ui_text(&d).unwrap();
            assert_eq!(t.text, "テスト文字列");
            let e = set_ui_text(&d, "Downloading").unwrap();
            assert_eq!(e[..t.text_offset], d[..t.text_offset]);
            assert_eq!(parse_ui_text(&e).unwrap().text, "Downloading");
            assert_eq!(set_ui_text(&e, "テスト文字列").unwrap(), d);
        }
        // No calls and an empty name, as in the APK's scenes: `m_Text` at byte 252.
        assert_eq!(parse_ui_text(&ui_text("x", 0)).unwrap().text_offset, 252);
    }

    /// A typetree-less scene file: a GameObject, two Texts, a non-Text MonoBehaviour.
    pub fn scene(texts: &[&str]) -> Vec<u8> {
        use jdr_unity::serialized::{write_layout, Object, TypeEntry};
        let entry = |class: i32| {
            let mut w = W::new();
            w.i32(class);
            w.u8(0);
            w.i16(if class == CLASS_MONOBEHAVIOUR { 0 } else { -1 });
            if class == CLASS_MONOBEHAVIOUR {
                w.bytes(&[7; 16]);
            }
            w.bytes(&[9; 16]);
            TypeEntry {
                class_id: class,
                raw: w.buf,
            }
        };
        let mut objects = vec![Object {
            path_id: 1,
            type_index: 0,
            data: vec![1, 2, 3, 4, 5],
        }];
        for (i, t) in texts.iter().enumerate() {
            objects.push(Object {
                path_id: 10 + i as i64,
                type_index: 1,
                data: ui_text(t, 0),
            });
        }
        objects.push(Object {
            path_id: 99,
            type_index: 1,
            data: vec![0; 40],
        });
        let order: Vec<usize> = (0..objects.len()).rev().collect();
        write_layout(
            jdr_unity::UNITY_VERSION,
            jdr_unity::PLATFORM_ANDROID,
            &[entry(1), entry(CLASS_MONOBEHAVIOUR)],
            &objects,
            &order,
            &[(0, 2591)],
            &0i32.to_le_bytes(),
            "",
            4096,
            false,
        )
    }

    #[test]
    fn scene_rewrite_touches_only_the_listed_texts() {
        let file = scene(&["テスト一", "テスト二"]);
        assert_eq!(
            scene_texts(&file).unwrap(),
            vec![(10, "テスト一".to_string()), (11, "テスト二".to_string())]
        );
        let (same, n) = rewrite_scene(&file, &|_, _| None).unwrap();
        assert_eq!((same == file, n), (true, 0));
        let (out, n) = rewrite_scene(&file, &|pid, t| {
            (pid == 11 && t == "テスト二").then(|| "No, thanks".to_string())
        })
        .unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            scene_texts(&out).unwrap(),
            vec![(10, "テスト一".to_string()), (11, "No, thanks".to_string())]
        );
        let (f, g) = (sfile::parse(&file).unwrap(), sfile::parse(&out).unwrap());
        assert_eq!(f.objects[0].data, g.objects[0].data);
        assert_eq!(f.objects[3].data, g.objects[3].data);
        // And back again: byte-identical to the original.
        let (back, _) =
            rewrite_scene(&out, &|pid, _| (pid == 11).then(|| "テスト二".into())).unwrap();
        assert_eq!(back, file);
    }

    /// Real APK (JDR_APK): every scene parses and round-trips; the Japanese labels are found.
    #[test]
    fn real_scene_texts() {
        let Ok(apk) = std::env::var("JDR_APK") else {
            eprintln!("skipped: set JDR_APK");
            return;
        };
        let mut z = zip::ZipArchive::new(std::fs::File::open(apk).unwrap()).unwrap();
        let names: Vec<String> = z
            .file_names()
            .filter(|n| is_scene(n))
            .map(str::to_owned)
            .collect();
        assert!(names.len() > 20, "{names:?}");
        let (mut texts, mut japanese) = (0, 0);
        for n in names {
            let mut file = Vec::new();
            std::io::Read::read_to_end(&mut z.by_name(&n).unwrap(), &mut file).unwrap();
            let t = scene_texts(&file).unwrap();
            texts += t.len();
            japanese += t
                .iter()
                .filter(|(_, s)| jdr_services::l10n::has_japanese(s))
                .count();
            let (same, k) = rewrite_scene(&file, &|_, _| None).unwrap();
            assert_eq!((same == file, k), (true, 0), "{n}");
        }
        eprintln!("{texts} UI.Text, {japanese} with Japanese");
        assert_eq!(japanese, 72);
    }

    #[test]
    fn ui_text_refuses_other_objects() {
        let d = ui_text("abc", 0);
        assert!(parse_ui_text(&d[..d.len() - 4]).is_err());
        let mut extra = d.clone();
        extra.extend_from_slice(&[0; 4]);
        assert!(parse_ui_text(&extra).is_err());
        let mut img = d.clone();
        img[72] = b'X'; // first byte of the event type name
        assert!(parse_ui_text(&img).is_err());
        assert!(parse_ui_text(&[0; 300]).is_err());
    }
}
