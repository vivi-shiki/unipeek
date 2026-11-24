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
    /// A `Reader` source after its only read.
    Consumed,
}

impl Source {
    /// Open a fresh reader. A `Reader` source can only be opened once; later calls fail.
    pub fn open(&mut self) -> io::Result<Box<dyn Read + Send + '_>> {
        match self {
            Source::Path(p) => Ok(Box::new(io::BufReader::with_capacity(1 << 20, std::fs::File::open(p)?))),
            Source::Bytes(b) => Ok(Box::new(io::Cursor::new(b.clone()))),
            Source::Reader(_) => {
                let Source::Reader(r) = std::mem::replace(self, Source::Consumed) else { unreachable!() };
                Ok(r)
            }
            Source::Consumed => Err(io::Error::other("the reader of this source was already consumed")),
        }
    }

    /// Whether `open` can be called again after the first read.
    pub fn reopenable(&self) -> bool {
        !matches!(self, Source::Reader(_) | Source::Consumed)
    }

    /// Short description for reports and errors. Never includes the bytes.
    pub fn describe(&self) -> String {
        match self {
            Source::Path(p) => p.display().to_string(),
            Source::Bytes(b) => format!("<{} bytes>", b.len()),
            Source::Reader(_) | Source::Consumed => "<reader>".into(),
        }
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
    pub fn parse_location(location: &str) -> (PathBuf, Vec<String>) {
        let mut parts = Vec::new();
        let mut cur = String::new();
        for (i, piece) in location.split('#').enumerate() {
            if i > 0 {
                cur.push('#');
            }
            cur.push_str(piece);
            if is_container(&cur) {
                parts.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            match parts.last_mut() {
                // `#` right after a container starts the next inner path; the separator itself is dropped.
                Some(_) if cur.starts_with('#') => parts.push(cur[1..].to_string()),
                Some(last) => last.push_str(&cur),
                None => parts.push(cur),
            }
        }
        let mut parts = parts.into_iter();
        let path = PathBuf::from(parts.next().unwrap_or_default());
        (path, parts.map(|p| p.strip_prefix('#').unwrap_or(&p).to_string()).collect())
    }

    pub fn from_location(location: &str) -> Input {
        let (path, inner) = Input::parse_location(location);
        Input { source: Source::Path(path), inner }
    }
}

pub(crate) fn is_container(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".unitypackage") || lower.ends_with(".zip")
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
