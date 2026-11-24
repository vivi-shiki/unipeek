//! Builders for synthetic packages. Real samples are paid assets and never go into tests that run in CI.
use std::io::Write;

/// Builds a `.unitypackage` (gzip tar) in memory.
#[derive(Default)]
pub struct PackageBuilder {
    files: Vec<(String, Vec<u8>)>,
    dot_slash: bool,
}

impl PackageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prefix every entry name with `./`, as some exporters do.
    pub fn dot_slash(mut self) -> Self {
        self.dot_slash = true;
        self
    }

    /// Add an asset with its `pathname`, in Unity's usual order: `asset`, `asset.meta`, `pathname`.
    pub fn asset(self, guid: &str, pathname: &str, asset: impl Into<Vec<u8>>) -> Self {
        let meta = format!("fileFormatVersion: 2\nguid: {guid}\n");
        self.asset_with_meta(guid, pathname, asset, meta)
    }

    pub fn asset_with_meta(mut self, guid: &str, pathname: &str, asset: impl Into<Vec<u8>>, meta: impl Into<Vec<u8>>) -> Self {
        self.files.push((format!("{guid}/asset"), asset.into()));
        self.files.push((format!("{guid}/asset.meta"), meta.into()));
        self.files.push((format!("{guid}/pathname"), pathname.as_bytes().to_vec()));
        self
    }

    /// A folder asset: `pathname` and `asset.meta`, no `asset`.
    pub fn folder(mut self, guid: &str, pathname: &str) -> Self {
        self.files.push((format!("{guid}/asset.meta"), format!("fileFormatVersion: 2\nguid: {guid}\nfolderAsset: yes\n").into_bytes()));
        self.files.push((format!("{guid}/pathname"), pathname.as_bytes().to_vec()));
        self
    }

    /// Any tar entry, for broken or hostile packages.
    pub fn raw(mut self, name: &str, data: impl Into<Vec<u8>>) -> Self {
        self.files.push((name.to_string(), data.into()));
        self
    }

    pub fn build(&self) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, data) in &self.files {
            let name = if self.dot_slash { format!("./{name}") } else { name.clone() };
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            tar.append_data(&mut header, &name, &data[..]).expect("tar entry");
        }
        let tar = tar.into_inner().expect("tar finish");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).expect("gzip");
        gz.finish().expect("gzip finish")
    }
}

/// A Unity YAML material referencing a shader and a main texture.
pub fn material_yaml(name: &str, shader_guid: &str, shader_file_id: i64, main_tex: Option<&str>, queue: i32) -> String {
    let tex = match main_tex {
        Some(g) => format!("{{fileID: 2800000, guid: {g}, type: 3}}"),
        None => "{fileID: 0}".into(),
    };
    format!(
        "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!21 &2100000\nMaterial:\n  serializedVersion: 8\n  m_Name: {name}\n  m_Shader: {{fileID: {shader_file_id}, guid: {shader_guid}, type: 3}}\n  m_CustomRenderQueue: {queue}\n  m_SavedProperties:\n    serializedVersion: 3\n    m_TexEnvs:\n    - _BumpMap:\n        m_Texture: {{fileID: 0}}\n        m_Scale: {{x: 1, y: 1}}\n        m_Offset: {{x: 0, y: 0}}\n    - _MainTex:\n        m_Texture: {tex}\n        m_Scale: {{x: 1, y: 1}}\n        m_Offset: {{x: 0, y: 0}}\n    m_Ints: []\n    m_Floats:\n    - _Cull: 2\n    - _Cutoff: 0.5\n    - _OutlineWidth: 0.08\n    m_Colors:\n    - _Color: {{r: 1, g: 0.5, b: 0.25, a: 1}}\n    - _OutlineColor: {{r: 0.2, g: 0.1, b: 0.1, a: 1}}\n    - _ShadowColor: {{r: 0.8, g: 0.7, b: 0.9, a: 1}}\n"
    )
}

/// FBX `.meta` mapping FBX material names to `.mat` GUIDs through `externalObjects`.
pub fn model_meta(guid: &str, materials: &[(&str, &str)]) -> String {
    let mut s = format!("fileFormatVersion: 2\nguid: {guid}\nModelImporter:\n  serializedVersion: 22200\n  internalIDToNameTable: []\n");
    if materials.is_empty() {
        s += "  externalObjects: {}\n";
    } else {
        s += "  externalObjects:\n";
    }
    for (name, mat) in materials {
        s += &format!(
            "  - first:\n      type: UnityEngine:Material\n      assembly: UnityEngine.CoreModule\n      name: {name}\n    second: {{fileID: 2100000, guid: {mat}, type: 2}}\n"
        );
    }
    s += "  meshes:\n    globalScale: 1\n";
    s
}

/// A prefab that instantiates a model (FBX) prefab, optionally overriding renderer materials.
pub fn prefab_instancing(model_guid: &str, overrides: &[(i64, usize, &str)]) -> String {
    let mut s = String::from(
        "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1001 &100100000001\nPrefabInstance:\n  m_ObjectHideFlags: 0\n  serializedVersion: 2\n  m_Modification:\n    serializedVersion: 3\n    m_TransformParent: {fileID: 0}\n    m_Modifications:\n",
    );
    s += &format!("    - target: {{fileID: 919132149155446097, guid: {model_guid},\n        type: 3}}\n      propertyPath: m_Name\n      value: Root\n      objectReference: {{fileID: 0}}\n");
    for (target, slot, mat) in overrides {
        s += &format!(
            "    - target: {{fileID: {target}, guid: {model_guid},\n        type: 3}}\n      propertyPath: m_Materials.Array.data[{slot}]\n      value: \n      objectReference: {{fileID: 2100000, guid: {mat}, type: 2}}\n"
        );
    }
    s += &format!("    m_RemovedComponents: []\n  m_SourcePrefab: {{fileID: 100100000, guid: {model_guid}, type: 3}}\n");
    s
}

/// A prefab with a plain GameObject holding a SkinnedMeshRenderer that points at a mesh in a model.
pub fn prefab_with_renderer(name: &str, mesh_guid: &str, materials: &[&str]) -> String {
    let mats: String = materials.iter().map(|g| format!("  - {{fileID: 2100000, guid: {g}, type: 2}}\n")).collect();
    format!(
        "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1 &10\nGameObject:\n  m_Component:\n  - component: {{fileID: 11}}\n  - component: {{fileID: 12}}\n  m_Name: {name}\n  m_IsActive: 1\n--- !u!4 &11\nTransform:\n  m_GameObject: {{fileID: 10}}\n  m_Children: []\n  m_Father: {{fileID: 0}}\n--- !u!137 &12\nSkinnedMeshRenderer:\n  m_GameObject: {{fileID: 10}}\n  m_Enabled: 1\n  m_Materials:\n{mats}  m_Mesh: {{fileID: 4300000, guid: {mesh_guid}, type: 3}}\n"
    )
}

/// A stored (uncompressed) zip with raw member names. `utf8` sets the language-encoding flag;
/// without it Japanese tools write Shift-JIS names.
pub fn zip(entries: &[(&[u8], &[u8], bool)]) -> Vec<u8> {
    let flagged: Vec<(&[u8], &[u8], u16)> = entries.iter().map(|(n, d, utf8)| (*n, *d, if *utf8 { 0x0800 } else { 0 })).collect();
    zip_with_flags(&flagged)
}

/// Like `zip`, with raw general-purpose flags (bit 0 marks an encrypted member).
pub fn zip_with_flags(entries: &[(&[u8], &[u8], u16)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for &(name, data, flags) in entries {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let offset = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&0u32.to_le_bytes());
            v.extend_from_slice(&crc.sum().to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        common(&mut out);
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        central.extend_from_slice(&0x02014b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        common(&mut central);
        central.extend_from_slice(&[0; 6]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// An ASCII FBX 7.4 with one mesh: one unit quad per material, side by side along +X, facing +Z.
/// Units are meters and Y is up.
pub fn fbx_ascii(materials: &[&str]) -> String {
    let n = materials.len().max(1);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut uv_index = Vec::new();
    let mut mats = Vec::new();
    for q in 0..n {
        let x = q as f32 * 1.5;
        for (vx, vy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            vertices.push(format!("{},{},0", x + vx, vy));
        }
        let b = (q * 4) as i64;
        indices.push(format!("{},{},{},{}", b, b + 1, b + 2, -(b + 3) - 1));
        uv_index.push("0,1,2,3".to_string());
        mats.push(q.to_string());
    }
    let mut s = String::from(
        "; FBX 7.4.0 project file\nFBXHeaderExtension:  {\n\tFBXHeaderVersion: 1003\n\tFBXVersion: 7400\n}\nGlobalSettings:  {\n\tVersion: 1000\n\tProperties70:  {\n\t\tP: \"UpAxis\", \"int\", \"Integer\", \"\",1\n\t\tP: \"UpAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"FrontAxis\", \"int\", \"Integer\", \"\",2\n\t\tP: \"FrontAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"CoordAxis\", \"int\", \"Integer\", \"\",0\n\t\tP: \"CoordAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"UnitScaleFactor\", \"double\", \"Number\", \"\",100\n\t}\n}\nObjects:  {\n",
    );
    s += &format!(
        "\tGeometry: 1000, \"Geometry::Quads\", \"Mesh\" {{\n\t\tVertices: *{} {{\n\t\t\ta: {}\n\t\t}}\n\t\tPolygonVertexIndex: *{} {{\n\t\t\ta: {}\n\t\t}}\n",
        n * 12,
        vertices.join(","),
        n * 4,
        indices.join(",")
    );
    s += &format!(
        "\t\tLayerElementUV: 0 {{\n\t\t\tVersion: 101\n\t\t\tName: \"UVMap\"\n\t\t\tMappingInformationType: \"ByPolygonVertex\"\n\t\t\tReferenceInformationType: \"IndexToDirect\"\n\t\t\tUV: *8 {{\n\t\t\t\ta: 0,0,1,0,1,1,0,1\n\t\t\t}}\n\t\t\tUVIndex: *{} {{\n\t\t\t\ta: {}\n\t\t\t}}\n\t\t}}\n",
        n * 4,
        uv_index.join(",")
    );
    s += &format!(
        "\t\tLayerElementMaterial: 0 {{\n\t\t\tVersion: 101\n\t\t\tName: \"\"\n\t\t\tMappingInformationType: \"ByPolygon\"\n\t\t\tReferenceInformationType: \"IndexToDirect\"\n\t\t\tMaterials: *{} {{\n\t\t\t\ta: {}\n\t\t\t}}\n\t\t}}\n",
        n,
        mats.join(",")
    );
    s += "\t\tLayer: 0 {\n\t\t\tVersion: 100\n\t\t\tLayerElement:  {\n\t\t\t\tType: \"LayerElementMaterial\"\n\t\t\t\tTypedIndex: 0\n\t\t\t}\n\t\t\tLayerElement:  {\n\t\t\t\tType: \"LayerElementUV\"\n\t\t\t\tTypedIndex: 0\n\t\t\t}\n\t\t}\n\t}\n";
    s += "\tModel: 2000, \"Model::Quads\", \"Mesh\" {\n\t\tVersion: 232\n\t}\n";
    for (i, m) in materials.iter().enumerate() {
        s += &format!("\tMaterial: {}, \"Material::{m}\", \"\" {{\n\t\tVersion: 102\n\t\tShadingModel: \"phong\"\n\t}}\n", 3000 + i);
    }
    s += "}\nConnections:  {\n\tC: \"OO\",1000,2000\n\tC: \"OO\",2000,0\n";
    for i in 0..materials.len() {
        s += &format!("\tC: \"OO\",{},2000\n", 3000 + i);
    }
    s += "}\n";
    s
}

/// An ASCII FBX 7.4 unit cube (edge 1 m, centered at the origin) with one material, for outlines.
pub fn fbx_ascii_cube(material: &str) -> String {
    let v = [(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)];
    let faces = [[4, 5, 6, 7], [1, 0, 3, 2], [5, 1, 2, 6], [0, 4, 7, 3], [7, 6, 2, 3], [0, 1, 5, 4]];
    let vertices: Vec<String> = v.iter().map(|(x, y, z)| format!("{},{},{}", *x as f32 * 0.5, *y as f32 * 0.5, *z as f32 * 0.5)).collect();
    let indices: Vec<String> = faces.iter().map(|f| format!("{},{},{},{}", f[0], f[1], f[2], -(f[3] as i64) - 1)).collect();
    // Start from the one-quad file and swap its geometry, UV indices and per-face materials.
    let quad = fbx_ascii(&[material]);
    let start = quad.find("\t\tVertices:").unwrap();
    let end = quad.find("\t\tLayerElementUV").unwrap();
    let geometry = format!(
        "\t\tVertices: *24 {{\n\t\t\ta: {}\n\t\t}}\n\t\tPolygonVertexIndex: *24 {{\n\t\t\ta: {}\n\t\t}}\n",
        vertices.join(","),
        indices.join(",")
    );
    let cube = format!("{}{}{}", &quad[..start], geometry, &quad[end..]);
    cube.replace("UVIndex: *4 {\n\t\t\t\ta: 0,1,2,3", &format!("UVIndex: *24 {{\n\t\t\t\ta: {}", ["0,1,2,3"; 6].join(",")))
        .replace("Materials: *1 {\n\t\t\t\ta: 0", "Materials: *6 {\n\t\t\t\ta: 0,0,0,0,0,0")
}

pub const FBX_BINARY_MAGIC: &[u8] = b"Kaydara FBX Binary  \x00\x1a\x00";
pub const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
