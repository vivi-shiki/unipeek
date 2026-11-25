//! Local file IDs of the objects Unity creates when it imports a model (2019.3 and later).
//!
//! Prefabs point at a renderer inside an FBX by `{fileID: N, guid: <fbx>}`. Unity derives `N` from
//! the object's type and place in the hierarchy: xxHash64 (seed 0) of
//! `Type:<type>-><identifier><index>`, read as a signed 64-bit integer. Meshes are identified by
//! name, everything in the hierarchy by a path that starts at `//RootNode/root`, the model prefab's
//! root. A model with a single top-level node merges that node into the root. Components add their
//! type name as the last path segment. The index counts objects with the same key and is 0 for
//! almost everything. Reverse-engineered and described in <https://awaken3d.dev/blog/computing-unitys-fileids>;
//! checked here against IDs from real imports.

/// The file ID of an object Unity registers under `key` (e.g. `Type:Mesh->Body0`).
pub fn file_id(key: &str) -> i64 {
    xxh64(key.as_bytes(), 0) as i64
}

/// The identifier path of a node, from the names of the nodes between the FBX scene root and it.
/// `top_level` is how many top-level nodes the FBX has.
pub fn node_path<S: AsRef<str>>(names: &[S], top_level: usize) -> String {
    let skip = usize::from(top_level == 1);
    let mut path = String::from("//RootNode/root");
    for n in names.iter().skip(skip) {
        path.push('/');
        path.push_str(n.as_ref());
    }
    path
}

/// File ID of the GameObject at `path` (from `node_path`).
pub fn game_object(path: &str) -> i64 {
    file_id(&format!("Type:GameObject->{path}0"))
}

/// File ID of a component (`SkinnedMeshRenderer`, `MeshRenderer`, `Transform`, ...) on the GameObject at `path`.
pub fn component(path: &str, type_name: &str) -> i64 {
    file_id(&format!("Type:{type_name}->{path}/{type_name}0"))
}

/// File ID of the mesh named `name`.
pub fn mesh(name: &str) -> i64 {
    file_id(&format!("Type:Mesh->{name}0"))
}

const P1: u64 = 11400714785074694791;
const P2: u64 = 14029467366897019727;
const P3: u64 = 1609587929392839161;
const P4: u64 = 9650029242287828579;
const P5: u64 = 2870177450012600261;

fn round(acc: u64, input: u64) -> u64 {
    acc.wrapping_add(input.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1)
}

fn merge(acc: u64, v: u64) -> u64 {
    (acc ^ round(0, v)).wrapping_mul(P1).wrapping_add(P4)
}

fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().expect("8 bytes"))
}

/// xxHash64 (public domain algorithm by Yann Collet), small inputs only need to be correct, not fast.
pub(crate) fn xxh64(b: &[u8], seed: u64) -> u64 {
    let n = b.len();
    let mut i = 0;
    let mut h = if n >= 32 {
        let mut v = [seed.wrapping_add(P1).wrapping_add(P2), seed.wrapping_add(P2), seed, seed.wrapping_sub(P1)];
        while i + 32 <= n {
            for (k, v) in v.iter_mut().enumerate() {
                *v = round(*v, u64_at(b, i + 8 * k));
            }
            i += 32;
        }
        let mut h = v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7)).wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18));
        for v in v {
            h = merge(h, v);
        }
        h
    } else {
        seed.wrapping_add(P5)
    };
    h = h.wrapping_add(n as u64);
    while i + 8 <= n {
        h = (h ^ round(0, u64_at(b, i))).rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        i += 8;
    }
    if i + 4 <= n {
        let v = u32::from_le_bytes(b[i..i + 4].try_into().expect("4 bytes")) as u64;
        h = (h ^ v.wrapping_mul(P1)).rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        i += 4;
    }
    for &byte in &b[i..] {
        h = (h ^ (byte as u64).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh64_reference_values() {
        assert_eq!(xxh64(b"", 0), 0xef46db3751d8e999);
        assert_eq!(xxh64(b"a", 0), 0xd24ec4f1a98c6e5b);
        assert_eq!(xxh64(b"abc", 0), 0x44bc2cf5ad770999);
        let long = b"Nobody inspects the spammish repetition";
        assert_eq!(xxh64(long, 0), 0xfbcea83c8a378bf1);
    }

    #[test]
    fn ids_unity_writes_for_every_model() {
        // Every model prefab's root GameObject and Transform have these IDs.
        assert_eq!(game_object(&node_path::<&str>(&[], 2)), 919132149155446097);
        assert_eq!(component("//RootNode/root", "Transform"), -8679921383154817045);
    }

    #[test]
    fn ids_from_real_imports() {
        // Values read from a prefab and the import artifact of an avatar FBX with many top-level nodes.
        let path = node_path(&["Hair_Front"], 23);
        assert_eq!(path, "//RootNode/root/Hair_Front");
        assert_eq!(game_object(&path), 6697033375267252120);
        assert_eq!(component(&path, "SkinnedMeshRenderer"), -4423955058006217718);
        assert_eq!(mesh("Cat_Ear"), -7622828335212821583);
    }

    #[test]
    fn a_single_top_level_node_is_merged_into_the_root() {
        assert_eq!(node_path(&["Armature", "Hips"], 1), "//RootNode/root/Hips");
        assert_eq!(node_path(&["Armature", "Hips"], 2), "//RootNode/root/Armature/Hips");
    }
}
