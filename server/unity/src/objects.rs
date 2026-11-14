//! Object data for the two engine classes every bundle we build carries (2018.4 layouts; their
//! type-tree entries are copied from the player's own bundles).

use crate::writer::W;

/// `MonoScript`: names the C# class an object's `m_Script` points at.
pub fn monoscript_data(class: &str, namespace: &str, properties_hash: [u8; 16]) -> Vec<u8> {
    let mut w = W::new();
    w.string(class); // m_Name
    w.i32(0); // m_ExecutionOrder
    w.bytes(&properties_hash); // m_PropertiesHash
    w.string(class); // m_ClassName
    w.string(namespace);
    w.string("Assembly-CSharp.dll");
    w.buf
}

/// One `m_Container` entry: lowercase asset path, its slice of the preload table, the object.
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    pub path: String,
    pub preload_index: i32,
    pub preload_size: i32,
    pub path_id: i64,
}

/// `AssetBundle` object (all references are local, file id 0). `container` must be sorted by
/// path, as the engine writes it.
pub fn assetbundle_data(name: &str, preload: &[i64], container: &[ContainerEntry]) -> Vec<u8> {
    let mut w = W::new();
    w.string(name); // m_Name
    w.i32(preload.len() as i32); // m_PreloadTable
    for id in preload {
        w.pptr(0, *id);
    }
    w.i32(container.len() as i32); // m_Container
    for c in container {
        w.string(&c.path);
        w.i32(c.preload_index);
        w.i32(c.preload_size);
        w.pptr(0, c.path_id);
    }
    w.i32(0); // m_MainAsset.preloadIndex
    w.i32(0); // m_MainAsset.preloadSize
    w.pptr(0, 0);
    w.u32(1); // m_RuntimeCompatibility
    w.string(name); // m_AssetBundleName
    w.i32(0); // m_Dependencies
    w.bool_aligned(false); // m_IsStreamedSceneAssetBundle
    w.i32(0); // m_ExplicitDataLayout
    w.i32(7); // m_PathFlags
    w.i32(0); // m_SceneHashes
    w.buf
}
