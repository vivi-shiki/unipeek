//! Zip and 7z archives behind one interface: list the members, stream one, or stream several in
//! archive order. 7z archives are usually solid, so reading a member decompresses what precedes it
//! in its block; callers read what they need in one `each` instead of member by member.
use crate::{Error, Result};
use std::{
    io::{self, Read, Seek},
    path::PathBuf,
    sync::Arc,
};

/// Where an archive's bytes are. Archives need seeking, so one-shot readers are read into memory.
#[derive(Clone)]
pub(crate) enum ArchiveData {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
}

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

impl ArchiveData {
    fn reader(&self) -> Result<Box<dyn ReadSeek>> {
        Ok(match self {
            ArchiveData::Path(p) => Box::new(io::BufReader::new(std::fs::File::open(p)?)),
            ArchiveData::Bytes(b) => Box::new(io::Cursor::new(b.clone())),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Zip,
    SevenZ,
}

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub index: usize,
    /// Path inside the archive with `/`; zip names without the UTF-8 flag decoded as Shift-JIS.
    pub name: String,
    pub size: u64,
}

#[derive(Clone)]
pub(crate) struct Archive {
    pub data: ArchiveData,
    pub format: Format,
}

impl Archive {
    /// Files of the archive (folders left out), in archive order.
    pub fn members(&self) -> Result<Vec<Member>> {
        match self.format {
            Format::Zip => {
                let mut zip = self.zip()?;
                let mut out = Vec::new();
                for i in 0..zip.len() {
                    let Ok(f) = zip.by_index_raw(i) else { continue };
                    if !f.is_dir() {
                        out.push(Member { index: i, name: decode_name(f.name_raw()), size: f.size() });
                    }
                }
                Ok(out)
            }
            Format::SevenZ => {
                let reader = self.seven()?;
                Ok(reader
                    .archive()
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| !e.is_directory && !e.is_anti_item)
                    .map(|(i, e)| Member { index: i, name: e.name.replace('\\', "/"), size: e.size })
                    .collect())
            }
        }
    }

    /// Stream the members `wanted` says yes to, in archive order. `f` may read any part of each.
    pub fn each(&self, mut wanted: impl FnMut(&Member) -> bool, mut f: impl FnMut(&Member, &mut dyn Read) -> Result<()>) -> Result<()> {
        match self.format {
            Format::Zip => {
                let mut zip = self.zip()?;
                for m in self.members()? {
                    if !wanted(&m) {
                        continue;
                    }
                    let mut file = zip.by_index(m.index).map_err(|e| Error::Unsupported(format!("zip member {}: {e}", m.name)))?;
                    f(&m, &mut file)?;
                }
                Ok(())
            }
            Format::SevenZ => {
                let mut reader = self.seven()?;
                let names: Vec<(String, u64)> = reader.archive().files.iter().map(|e| (e.name.replace('\\', "/"), e.size)).collect();
                let mut index = 0usize;
                let mut failure = None;
                let result = reader.for_each_entries(|entry, r| {
                    let i = names.iter().skip(index).position(|(n, _)| *n == entry.name.replace('\\', "/")).map_or(index, |p| index + p);
                    index = i + 1;
                    if entry.is_directory {
                        return Ok(true);
                    }
                    let m = Member { index: i, name: entry.name.replace('\\', "/"), size: entry.size };
                    if wanted(&m)
                        && let Err(e) = f(&m, r)
                    {
                        failure = Some(e);
                        return Ok(false);
                    }
                    Ok(true)
                });
                if let Some(e) = failure {
                    return Err(e);
                }
                result.map_err(|e| Error::Unsupported(format!("unreadable 7z: {e}")))
            }
        }
    }

    /// Stream one member.
    pub fn with_member<T>(&self, member: &Member, f: impl FnOnce(&mut dyn Read) -> Result<T>) -> Result<T> {
        let mut f = Some(f);
        let mut out = None;
        let name = member.name.clone();
        self.each(
            |m| m.name == name,
            |_, r| {
                if let Some(f) = f.take() {
                    out = Some(f(r)?);
                }
                Ok(())
            },
        )?;
        out.ok_or_else(|| Error::NotFound(format!("{name} is not in the archive")))
    }

    fn zip(&self) -> Result<zip::ZipArchive<Box<dyn ReadSeek>>> {
        zip::ZipArchive::new(self.data.reader()?).map_err(|e| Error::Unsupported(format!("unreadable zip: {e}")))
    }

    fn seven(&self) -> Result<sevenz_rust2::ArchiveReader<Box<dyn ReadSeek>>> {
        sevenz_rust2::ArchiveReader::new(self.data.reader()?, sevenz_rust2::Password::empty()).map_err(|e| Error::Unsupported(format!("unreadable 7z: {e}")))
    }
}

/// Zip names are UTF-8 when valid, otherwise Shift-JIS (Japanese tools do not set the UTF-8 flag).
pub(crate) fn decode_name(raw: &[u8]) -> String {
    let name = match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::SHIFT_JIS.decode(raw).0.into_owned(),
    };
    name.replace('\\', "/")
}
