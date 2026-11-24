//! A small reader for the subset of YAML that Unity writes (`.meta`, `.mat`, `.prefab`).
//!
//! Unity YAML is a stream of documents headed `--- !u!CLASS &FILEID`, with block mappings,
//! sequences written at the same indent as their key, and flow mappings such as
//! `{fileID: 2100000, guid: …, type: 2}` that may wrap over two lines.
use crate::Guid;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Null,
    Scalar(String),
    Map(Vec<(String, Node)>),
    Seq(Vec<Node>),
}

/// `{fileID: N, guid: G, type: T}`: a reference to an object in this file or in another asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjRef {
    pub file_id: i64,
    pub guid: Option<Guid>,
    pub kind: Option<i32>,
}

impl ObjRef {
    /// `{fileID: 0}` points at nothing.
    pub fn is_null(&self) -> bool {
        self.file_id == 0 && self.guid.is_none_or(|g| g.is_zero())
    }
}

impl Node {
    pub fn get(&self, _key: &str) -> Option<&Node> {
        todo!()
    }

    /// Follow a chain of keys.
    pub fn path(&self, keys: &[&str]) -> Option<&Node> {
        keys.iter().try_fold(self, |n, k| n.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        todo!()
    }

    pub fn as_i64(&self) -> Option<i64> {
        todo!()
    }

    pub fn as_f32(&self) -> Option<f32> {
        todo!()
    }

    /// Items of a sequence; an empty slice for anything else (Unity writes `[]` and `{}` for empty).
    pub fn items(&self) -> &[Node] {
        todo!()
    }

    pub fn entries(&self) -> &[(String, Node)] {
        todo!()
    }

    pub fn reference(&self) -> Option<ObjRef> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub class_id: u32,
    pub file_id: i64,
    /// `stripped` documents only stand in for an object of a nested prefab.
    pub stripped: bool,
    /// The single top-level key, e.g. `Material` or `PrefabInstance`.
    pub type_name: String,
    pub body: Node,
}

/// Parse a Unity YAML file into its documents. Lines that do not fit are skipped, never fatal.
pub fn parse_documents(_text: &str) -> Vec<Document> {
    todo!()
}

/// Parse a file without document headers (`.meta`).
pub fn parse(_text: &str) -> Node {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAT: &str = "%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!21 &2100000
Material:
  serializedVersion: 8
  m_Name: Body skin
  m_Shader: {fileID: 4800000, guid: efa77a80ca0344749b4f19fdd5891cbe, type: 3}
  m_ValidKeywords: []
  stringTagMap:
    VRCFallback: Toon
  m_SavedProperties:
    serializedVersion: 3
    m_TexEnvs:
    - _MainTex:
        m_Texture: {fileID: 2800000, guid: 4bbb1657292f0d74dab36e002aa5696d, type: 3}
        m_Scale: {x: 1, y: 1}
        m_Offset: {x: 0, y: 0}
    - _BumpMap:
        m_Texture: {fileID: 0}
    m_Floats:
    - _Cull: 2
    - _Cutoff: 0.5
    m_Colors:
    - _Color: {r: 1, g: 0.5, b: 0.25, a: 1}
";

    #[test]
    fn document_header_and_scalars() {
        let docs = parse_documents(MAT);
        assert_eq!(docs.len(), 1);
        let d = &docs[0];
        assert_eq!((d.class_id, d.file_id, d.stripped), (21, 2100000, false));
        assert_eq!(d.type_name, "Material");
        assert_eq!(d.body.get("m_Name").and_then(Node::as_str), Some("Body skin"));
        assert_eq!(d.body.get("serializedVersion").and_then(Node::as_i64), Some(8));
        assert_eq!(d.body.path(&["stringTagMap", "VRCFallback"]).and_then(Node::as_str), Some("Toon"));
        assert!(d.body.get("m_ValidKeywords").unwrap().items().is_empty());
    }

    #[test]
    fn flow_mapping_is_a_reference() {
        let d = &parse_documents(MAT)[0];
        let r = d.body.get("m_Shader").unwrap().reference().unwrap();
        assert_eq!(r.file_id, 4800000);
        assert_eq!(r.guid, Guid::parse("efa77a80ca0344749b4f19fdd5891cbe"));
        assert_eq!(r.kind, Some(3));
        assert!(!r.is_null());
    }

    #[test]
    fn sequences_at_key_indent_with_nested_maps() {
        let d = &parse_documents(MAT)[0];
        let envs = d.body.path(&["m_SavedProperties", "m_TexEnvs"]).unwrap().items();
        assert_eq!(envs.len(), 2);
        let (name, tex) = &envs[0].entries()[0];
        assert_eq!(name, "_MainTex");
        assert_eq!(tex.path(&["m_Texture"]).unwrap().reference().unwrap().file_id, 2800000);
        assert_eq!(tex.path(&["m_Scale", "x"]).and_then(Node::as_f32), Some(1.0));
        assert!(envs[1].entries()[0].1.get("m_Texture").unwrap().reference().unwrap().is_null());
        let floats = d.body.path(&["m_SavedProperties", "m_Floats"]).unwrap().items();
        assert_eq!(floats[1].entries()[0].0, "_Cutoff");
        assert_eq!(floats[1].entries()[0].1.as_f32(), Some(0.5));
        let color = &d.body.path(&["m_SavedProperties", "m_Colors"]).unwrap().items()[0].entries()[0].1;
        assert_eq!(color.get("g").and_then(Node::as_f32), Some(0.5));
    }

    #[test]
    fn flow_mapping_wrapped_over_lines() {
        let text = "--- !u!1001 &3515145266248618786
PrefabInstance:
  m_Modification:
    m_Modifications:
    - target: {fileID: 249692165999412322, guid: 4f920a12728c5f44db851043221361cb,
        type: 3}
      propertyPath: m_Materials.Array.data[1]
      value:
      objectReference: {fileID: 2100000, guid: fd7c89bf2761e894eaf746251d625bab, type: 2}
    - target: {fileID: 1, guid: 4f920a12728c5f44db851043221361cb, type: 3}
      propertyPath: m_Name
      value: Kipfel
      objectReference: {fileID: 0}
  m_SourcePrefab: {fileID: 100100000, guid: 7078d86f15dd32d4bb4540ac2ff9e0ac, type: 3}
";
        let d = &parse_documents(text)[0];
        assert_eq!(d.class_id, 1001);
        let mods = d.body.path(&["m_Modification", "m_Modifications"]).unwrap().items();
        assert_eq!(mods.len(), 2);
        let t = mods[0].get("target").unwrap().reference().unwrap();
        assert_eq!(t.file_id, 249692165999412322);
        assert_eq!(t.kind, Some(3));
        assert_eq!(mods[0].get("propertyPath").and_then(Node::as_str), Some("m_Materials.Array.data[1]"));
        assert_eq!(mods[0].get("value"), Some(&Node::Null));
        assert_eq!(mods[1].get("value").and_then(Node::as_str), Some("Kipfel"));
        assert_eq!(d.body.get("m_SourcePrefab").unwrap().reference().unwrap().file_id, 100100000);
    }

    #[test]
    fn several_documents_and_stripped() {
        let text = "%YAML 1.1
--- !u!1 &10
GameObject:
  m_Component:
  - component: {fileID: 11}
  - component: {fileID: 12}
  m_Name: Body
  m_IsActive: 0
--- !u!137 &12 stripped
SkinnedMeshRenderer:
  m_CorrespondingSourceObject: {fileID: 1370, guid: 7078d86f15dd32d4bb4540ac2ff9e0ac, type: 3}
--- !u!4 &-8679921383154817045
Transform:
  m_Children: []
";
        let docs = parse_documents(text);
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].body.get("m_Component").unwrap().items().len(), 2);
        assert_eq!(docs[0].body.get("m_Component").unwrap().items()[1].get("component").unwrap().reference().unwrap().file_id, 12);
        assert_eq!(docs[0].body.get("m_IsActive").and_then(Node::as_i64), Some(0));
        assert!(docs[1].stripped);
        assert_eq!(docs[1].class_id, 137);
        assert_eq!(docs[2].file_id, -8679921383154817045);
        assert!(docs[2].body.get("m_Children").unwrap().items().is_empty());
    }

    #[test]
    fn quoted_and_wrapped_scalars() {
        let text = "--- !u!1 &1
GameObject:
  m_Name: 'it''s: quoted'
  m_TagString: \"double \\\"q\\\"\"
  m_Long: a plain scalar that
    wraps onto the next line
  m_After: 1
";
        let d = &parse_documents(text)[0];
        assert_eq!(d.body.get("m_Name").and_then(Node::as_str), Some("it's: quoted"));
        assert_eq!(d.body.get("m_TagString").and_then(Node::as_str), Some("double \"q\""));
        assert_eq!(d.body.get("m_Long").and_then(Node::as_str), Some("a plain scalar that wraps onto the next line"));
        assert_eq!(d.body.get("m_After").and_then(Node::as_i64), Some(1));
    }

    #[test]
    fn meta_without_header() {
        let text = "fileFormatVersion: 2
guid: 7078d86f15dd32d4bb4540ac2ff9e0ac
ModelImporter:
  serializedVersion: 22200
  internalIDToNameTable: []
  externalObjects:
  - first:
      type: UnityEngine:Material
      assembly: UnityEngine.CoreModule
      name: Body
    second: {fileID: 2100000, guid: c5898e34be6905c45a38f3f063d0172d, type: 2}
  - first:
      type: UnityEngine:Material
      assembly: UnityEngine.CoreModule
      name: Hair
    second: {fileID: 2100000, guid: 00f0f13e2177deb409cf3f9658d0fb00, type: 2}
  materials:
    materialImportMode: 2
  meshes:
    globalScale: 1
";
        let n = parse(text);
        assert_eq!(n.get("guid").and_then(Node::as_str), Some("7078d86f15dd32d4bb4540ac2ff9e0ac"));
        let ext = n.path(&["ModelImporter", "externalObjects"]).unwrap().items();
        assert_eq!(ext.len(), 2);
        assert_eq!(ext[1].path(&["first", "name"]).and_then(Node::as_str), Some("Hair"));
        assert_eq!(ext[1].get("first").unwrap().get("type").and_then(Node::as_str), Some("UnityEngine:Material"));
        assert!(ext[0].get("second").unwrap().reference().unwrap().guid.is_some());
        assert_eq!(n.path(&["ModelImporter", "meshes", "globalScale"]).and_then(Node::as_f32), Some(1.0));
    }

    #[test]
    fn garbage_does_not_panic() {
        for text in ["", "---", "--- !u!x &y\n", ":\n  - - :\n{{{", "a: {b: [c, {d: e}]}\n", "- a\n- b\n", "k: [1, 2]\n"] {
            let _ = parse_documents(text);
            let _ = parse(text);
        }
        let n = parse("k: [1, 2]\nm: {a: 1}\n");
        assert_eq!(n.get("k").unwrap().items().len(), 2);
        assert_eq!(n.path(&["m", "a"]).and_then(Node::as_i64), Some(1));
    }
}
