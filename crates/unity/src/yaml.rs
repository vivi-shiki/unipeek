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
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries().iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Follow a chain of keys.
    pub fn path(&self, keys: &[&str]) -> Option<&Node> {
        keys.iter().try_fold(self, |n, k| n.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::Scalar(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.as_str()?.trim().parse().ok()
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_str()?.trim().parse().ok()
    }

    /// Items of a sequence; an empty slice for anything else (Unity writes `[]` and `{}` for empty).
    pub fn items(&self) -> &[Node] {
        match self {
            Node::Seq(v) => v,
            _ => &[],
        }
    }

    pub fn entries(&self) -> &[(String, Node)] {
        match self {
            Node::Map(v) => v,
            _ => &[],
        }
    }

    pub fn reference(&self) -> Option<ObjRef> {
        let file_id = self.get("fileID")?.as_i64()?;
        let guid = self.get("guid").and_then(Node::as_str).and_then(Guid::parse);
        let kind = self.get("type").and_then(Node::as_i64).map(|k| k as i32);
        Some(ObjRef { file_id, guid, kind })
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
pub fn parse_documents(text: &str) -> Vec<Document> {
    let mut docs = Vec::new();
    let mut header: Option<(u32, i64, bool)> = None;
    let mut body: Vec<&str> = Vec::new();
    let mut flush = |header: Option<(u32, i64, bool)>, body: &mut Vec<&str>| {
        if let Some((class_id, file_id, stripped)) = header {
            let node = parse_lines(&logical_lines(body.iter().copied()));
            let (type_name, body) = match node {
                Node::Map(mut entries) if !entries.is_empty() => entries.swap_remove(0),
                _ => (String::new(), Node::Null),
            };
            docs.push(Document { class_id, file_id, stripped, type_name, body });
        }
        body.clear();
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("---") {
            flush(header.take(), &mut body);
            header = parse_header(rest);
        } else if header.is_some() {
            body.push(line);
        }
    }
    flush(header, &mut body);
    docs
}

/// Parse a file without document headers (`.meta`).
pub fn parse(text: &str) -> Node {
    parse_lines(&logical_lines(text.lines().filter(|l| !l.starts_with("---"))))
}

/// ` !u!21 &2100000 stripped`
fn parse_header(rest: &str) -> Option<(u32, i64, bool)> {
    let mut parts = rest.split_whitespace();
    let class_id = parts.next()?.strip_prefix("!u!")?.parse().ok()?;
    let file_id = parts.next()?.strip_prefix('&')?.parse().ok()?;
    let stripped = parts.any(|p| p == "stripped");
    Some((class_id, file_id, stripped))
}

struct Line {
    indent: usize,
    text: String,
}

/// Physical lines to logical lines: drop directives and blanks, join flow collections and quoted
/// strings that wrap, and plain scalars continued on more-indented lines.
fn logical_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    // Indent a line must exceed to continue the plain scalar of the previous line.
    let mut continues_after: Option<usize> = None;
    let mut open = false;
    for raw in lines {
        if raw.starts_with('%') || raw.trim().is_empty() {
            continue;
        }
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let text = raw.trim();
        if let Some(last) = out.last_mut() {
            if open || continues_after.is_some_and(|i| indent > i) {
                last.text.push(' ');
                last.text.push_str(text);
                open = is_open(&last.text);
                continue;
            }
        }
        out.push(Line { indent, text: text.to_string() });
        let last = out.last().unwrap();
        open = is_open(&last.text);
        continues_after = plain_value_start(&last.text).map(|offset| indent + offset);
    }
    out
}

/// Offset of the key column if the line ends with a plain scalar value that could wrap.
fn plain_value_start(text: &str) -> Option<usize> {
    let (offset, body) = match text.strip_prefix("- ") {
        Some(rest) => (2, rest),
        None => (0, text),
    };
    let value = match split_key(body) {
        Some((_, v)) => v,
        None if offset == 2 => body,
        None => return None,
    };
    let first = value.chars().next()?;
    (!matches!(first, '{' | '[' | '\'' | '"' | '|' | '>')).then_some(offset)
}

/// Whether brackets or quotes are still open at the end of `text`.
fn is_open(text: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let value = split_key(text.strip_prefix("- ").unwrap_or(text)).map_or(text, |(_, v)| v);
    for c in value.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') if depth == 0 && value.trim_start().starts_with(c) => quote = Some(c),
            (None, '{' | '[') => depth += 1,
            (None, '}' | ']') => depth -= 1,
            _ => {}
        }
    }
    depth > 0 || quote.is_some()
}

/// `key: value` or `key:` outside quotes and brackets.
fn split_key(text: &str) -> Option<(&str, &str)> {
    if text.starts_with(['{', '[', '\'', '"', '-']) && !text.starts_with("-_") {
        return None;
    }
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' | b'[' | b'\'' | b'"' => return None,
            b':' if i + 1 == bytes.len() => return Some((&text[..i], "")),
            b':' if bytes[i + 1] == b' ' => return Some((&text[..i], text[i + 2..].trim())),
            _ => {}
        }
    }
    None
}

fn is_item(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

fn parse_lines(lines: &[Line]) -> Node {
    let mut lines: Vec<Line> = lines.iter().map(|l| Line { indent: l.indent, text: l.text.clone() }).collect();
    let mut i = 0;
    let Some(first) = lines.first() else { return Node::Null };
    let indent = first.indent;
    let node = parse_block(&mut lines, &mut i, indent);
    node
}

fn parse_block(lines: &mut [Line], i: &mut usize, indent: usize) -> Node {
    match lines.get(*i) {
        Some(l) if is_item(&l.text) => parse_seq(lines, i, indent),
        Some(_) => parse_map(lines, i, indent),
        None => Node::Null,
    }
}

fn parse_map(lines: &mut [Line], i: &mut usize, indent: usize) -> Node {
    let mut entries = Vec::new();
    while let Some(line) = lines.get(*i) {
        if line.indent < indent || (line.indent == indent && is_item(&line.text)) {
            break;
        }
        if line.indent > indent {
            *i += 1;
            continue;
        }
        let Some((key, value)) = split_key(&line.text) else {
            *i += 1;
            continue;
        };
        let (key, value) = (unquote(key), value.to_string());
        *i += 1;
        let node = if !value.is_empty() {
            parse_inline(&value)
        } else {
            match lines.get(*i) {
                Some(next) if next.indent > indent => {
                    let child = next.indent;
                    parse_block(lines, i, child)
                }
                Some(next) if next.indent == indent && is_item(&next.text) => parse_seq(lines, i, indent),
                _ => Node::Null,
            }
        };
        entries.push((key, node));
    }
    Node::Map(entries)
}

fn parse_seq(lines: &mut [Line], i: &mut usize, indent: usize) -> Node {
    let mut items = Vec::new();
    while let Some(line) = lines.get(*i) {
        if line.indent != indent || !is_item(&line.text) {
            if line.indent > indent {
                *i += 1;
                continue;
            }
            break;
        }
        let content = line.text.strip_prefix('-').unwrap_or("").trim_start().to_string();
        if content.is_empty() {
            *i += 1;
            let item = match lines.get(*i) {
                Some(next) if next.indent > indent => {
                    let child = next.indent;
                    parse_block(lines, i, child)
                }
                _ => Node::Null,
            };
            items.push(item);
        } else if split_key(&content).is_some() {
            // `- key: value` starts a mapping whose other keys sit two columns further in.
            lines[*i] = Line { indent: indent + 2, text: content };
            items.push(parse_map(lines, i, indent + 2));
        } else {
            *i += 1;
            items.push(parse_inline(&content));
        }
    }
    Node::Seq(items)
}

fn parse_inline(text: &str) -> Node {
    let text = text.trim();
    if text.is_empty() {
        return Node::Null;
    }
    if text.starts_with(['{', '[']) {
        let mut chars = text.char_indices().peekable();
        return Flow { text, chars: &mut chars }.value();
    }
    Node::Scalar(unquote(text))
}

fn unquote(text: &str) -> String {
    let t = text.trim();
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        t[1..t.len() - 1].replace("''", "'")
    } else if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        let mut out = String::new();
        let mut chars = t[1..t.len() - 1].chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        t.to_string()
    }
}

/// Recursive descent over `{a: b, c: [d, e]}`.
struct Flow<'a, 'b> {
    text: &'a str,
    chars: &'b mut std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl Flow<'_, '_> {
    fn skip_ws(&mut self) {
        while self.chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}
    }

    fn value(&mut self) -> Node {
        self.skip_ws();
        match self.chars.peek().map(|&(_, c)| c) {
            Some('{') => {
                self.chars.next();
                let mut entries = Vec::new();
                loop {
                    self.skip_ws();
                    match self.chars.peek().map(|&(_, c)| c) {
                        None => break,
                        Some('}') => {
                            self.chars.next();
                            break;
                        }
                        Some(',') => {
                            self.chars.next();
                        }
                        Some(_) => {
                            let key = self.scalar(&[':', ',', '}']);
                            self.skip_ws();
                            let value = if self.chars.next_if(|&(_, c)| c == ':').is_some() { self.value() } else { Node::Null };
                            entries.push((key, value));
                        }
                    }
                }
                Node::Map(entries)
            }
            Some('[') => {
                self.chars.next();
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.chars.peek().map(|&(_, c)| c) {
                        None => break,
                        Some(']') => {
                            self.chars.next();
                            break;
                        }
                        Some(',') => {
                            self.chars.next();
                        }
                        Some(_) => items.push(self.value()),
                    }
                }
                Node::Seq(items)
            }
            Some(_) => {
                let s = self.scalar(&[',', '}', ']']);
                if s.is_empty() { Node::Null } else { Node::Scalar(s) }
            }
            None => Node::Null,
        }
    }

    fn scalar(&mut self, stops: &[char]) -> String {
        self.skip_ws();
        let start = self.chars.peek().map_or(self.text.len(), |&(i, _)| i);
        let mut end = start;
        let mut quote: Option<char> = None;
        while let Some(&(i, c)) = self.chars.peek() {
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if (c == '\'' || c == '"') && i == start => quote = Some(c),
                // A colon only ends a key when followed by a space or a delimiter, like `a: b`.
                None if c == ':' && stops.contains(&':') => {
                    let next = self.text[i + 1..].chars().next();
                    if next.is_none_or(|n| n == ' ' || n == ',' || n == '}') {
                        break;
                    }
                }
                None if stops.contains(&c) => break,
                None => {}
            }
            self.chars.next();
            end = i + c.len_utf8();
        }
        unquote(&self.text[start..end])
    }
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
      value: Avatar
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
        assert_eq!(mods[1].get("value").and_then(Node::as_str), Some("Avatar"));
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
