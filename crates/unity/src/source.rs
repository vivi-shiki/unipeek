//! Where an asset comes from: a path, bytes or a one-shot reader, plus an optional path inside it.
use std::{
    io::{self, Read},
    path::PathBuf,
    sync::Arc,
};

/// The bytes of an input. Paths and bytes can be read more than once, a reader only once.
pub enum Source {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
    Reader(Box<dyn Read + Send>),
}

impl Source {
    /// Open a fresh reader. A `Reader` source can only be opened once; later calls fail.
    pub fn open(&mut self) -> io::Result<Box<dyn Read + Send + '_>> {
        todo!()
    }

    /// Whether `open` can be called again after the first read.
    pub fn reopenable(&self) -> bool {
        todo!()
    }

    /// Short description for reports and errors. Never includes the bytes.
    pub fn describe(&self) -> String {
        todo!()
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}

/// A source plus the chain of paths inside it, e.g. `a.zip#b.unitypackage#Assets/x.prefab`.
#[derive(Debug)]
pub struct Input {
    pub source: Source,
    pub inner: Vec<String>,
}

impl Input {
    pub fn new(source: Source) -> Input {
        Input { source, inner: Vec::new() }
    }

    /// Split a command line argument at `#` after a container (`.unitypackage`, `.zip`).
    /// A `#` inside a folder or file name is kept.
    pub fn parse_location(_location: &str) -> (PathBuf, Vec<String>) {
        todo!()
    }

    pub fn from_location(location: &str) -> Input {
        let (path, inner) = Input::parse_location(location);
        Input { source: Source::Path(path), inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn location_without_inner_path() {
        let (p, inner) = Input::parse_location("D:/a/Avatar.unitypackage");
        assert_eq!(p, PathBuf::from("D:/a/Avatar.unitypackage"));
        assert!(inner.is_empty());
    }

    #[test]
    fn location_with_inner_prefab() {
        let (p, inner) = Input::parse_location("D:/a/Avatar.unitypackage#Assets/Shop/Avatar.prefab");
        assert_eq!(p, PathBuf::from("D:/a/Avatar.unitypackage"));
        assert_eq!(inner, vec!["Assets/Shop/Avatar.prefab".to_string()]);
    }

    #[test]
    fn location_nested_in_zip() {
        let (p, inner) = Input::parse_location("x/Item.ZIP#Item/Avatar.unitypackage#Assets/A#1.prefab");
        assert_eq!(p, PathBuf::from("x/Item.ZIP"));
        assert_eq!(inner, vec!["Item/Avatar.unitypackage".to_string(), "Assets/A#1.prefab".to_string()]);
    }

    #[test]
    fn hash_inside_folder_name_is_kept() {
        let (p, inner) = Input::parse_location("C:/shop #2/Avatar.unitypackage");
        assert_eq!(p, PathBuf::from("C:/shop #2/Avatar.unitypackage"));
        assert!(inner.is_empty());
        let (p, inner) = Input::parse_location("C:/shop #2/model.fbx");
        assert_eq!(p, PathBuf::from("C:/shop #2/model.fbx"));
        assert!(inner.is_empty());
    }

    #[test]
    fn bytes_can_be_opened_twice_reader_once() {
        let mut s = Source::Bytes(Arc::from(&b"abc"[..]));
        assert!(s.reopenable());
        for _ in 0..2 {
            let mut buf = String::new();
            s.open().unwrap().read_to_string(&mut buf).unwrap();
            assert_eq!(buf, "abc");
        }
        let mut r = Source::Reader(Box::new(io::Cursor::new(b"xyz".to_vec())));
        assert!(!r.reopenable());
        let mut buf = String::new();
        r.open().unwrap().read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "xyz");
        assert!(r.open().is_err());
    }

    #[test]
    fn describe_does_not_dump_bytes() {
        assert_eq!(Source::Bytes(Arc::from(&b"abcdef"[..])).describe(), "<6 bytes>");
        assert_eq!(Source::Path(PathBuf::from("a/b.fbx")).describe(), "a/b.fbx");
        assert_eq!(Source::Reader(Box::new(io::empty())).describe(), "<reader>");
    }
}
