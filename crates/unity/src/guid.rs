//! Unity asset GUIDs: 32 lowercase hex digits.
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Guid([u8; 16]);

impl Guid {
    pub fn parse(s: &str) -> Option<Guid> {
        let b = s.as_bytes();
        if b.len() != 32 {
            return None;
        }
        let mut out = [0u8; 16];
        for (i, pair) in b.chunks_exact(2).enumerate() {
            out[i] = (hex(pair[0])? << 4) | hex(pair[1])?;
        }
        Some(Guid(out))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 16]
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({self})")
    }
}

impl FromStr for Guid {
    type Err = crate::Error;
    fn from_str(s: &str) -> crate::Result<Guid> {
        Guid::parse(s).ok_or_else(|| crate::Error::InvalidInput(format!("not a GUID: {s:?}")))
    }
}

impl serde::Serialize for Guid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_lowercase_hex() {
        let g = Guid::parse("7078D86F15DD32D4BB4540AC2FF9E0AC").unwrap();
        assert_eq!(g.to_string(), "7078d86f15dd32d4bb4540ac2ff9e0ac");
        assert_eq!(Guid::parse(&g.to_string()), Some(g));
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert_eq!(Guid::parse(""), None);
        assert_eq!(Guid::parse("7078d86f15dd32d4bb4540ac2ff9e0a"), None);
        assert_eq!(Guid::parse("7078d86f15dd32d4bb4540ac2ff9e0acc"), None);
        assert_eq!(Guid::parse("7078d86f15dd32d4bb4540ac2ff9e0ag"), None);
        assert_eq!(Guid::parse("Assets"), None);
    }

    #[test]
    fn zero_guid_is_detected() {
        assert!(Guid::parse("00000000000000000000000000000000").unwrap().is_zero());
        assert!(!Guid::parse("0000000000000000f000000000000000").unwrap().is_zero());
    }
}
