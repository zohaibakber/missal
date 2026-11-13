use std::collections::HashMap;
use std::io::{Cursor, Read};

use roxmltree::Document as XmlDocument;
use zip::ZipArchive;

use crate::ImportError;
use crate::xml::plain_attr;

const MAX_PART_BYTES: u64 = 256 * 1024 * 1024;
const OFFICE_DOCUMENT: &str = "/officeDocument";

pub struct Package<'b> {
    archive: ZipArchive<Cursor<&'b [u8]>>,
    names: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub kind: String,
    pub target: String,
    pub external: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Relationships {
    by_id: HashMap<String, Relationship>,
}

impl Relationships {
    pub fn get(&self, id: &str) -> Option<&Relationship> {
        self.by_id.get(id)
    }

    pub fn of_kind(&self, suffix: &str) -> Option<&Relationship> {
        let mut found: Vec<(&String, &Relationship)> = self
            .by_id
            .iter()
            .filter(|(_, relationship)| relationship.kind.ends_with(suffix))
            .collect();
        found.sort_by(|left, right| left.0.cmp(right.0));
        found.first().map(|(_, relationship)| *relationship)
    }
}

impl<'b> Package<'b> {
    pub fn open(bytes: &'b [u8]) -> Result<Package<'b>, ImportError> {
        let archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| ImportError::NotDocx)?;
        let names = (0..archive.len())
            .filter_map(|index| {
                let name = archive.name_for_index(index).ok()?;
                Some((name.trim_start_matches('/').to_ascii_lowercase(), index))
            })
            .collect();
        Ok(Package { archive, names })
    }

    pub fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        let index = *self
            .names
            .get(&name.trim_start_matches('/').to_ascii_lowercase())?;
        let file = self.archive.by_index(index).ok()?;
        let mut bytes = Vec::with_capacity(usize::try_from(file.size().min(MAX_PART_BYTES)).ok()?);
        file.take(MAX_PART_BYTES).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    pub fn read_text(&mut self, name: &str) -> Option<String> {
        let bytes = self.read(name)?;
        Some(decode_text(bytes))
    }

    pub fn relationships(&mut self, part: &str) -> Relationships {
        let (directory, file) = match part.rfind('/') {
            Some(slash) => (&part[..slash], &part[slash + 1..]),
            None => ("", part),
        };
        let rels_name = if directory.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{directory}/_rels/{file}.rels")
        };
        let Some(text) = self.read_text(&rels_name) else {
            return Relationships::default();
        };
        let Ok(xml) = XmlDocument::parse(&text) else {
            return Relationships::default();
        };
        let by_id = xml
            .root_element()
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "Relationship")
            .filter_map(|node| {
                let id = plain_attr(node, "Id")?;
                let kind = plain_attr(node, "Type").unwrap_or_default();
                let target = plain_attr(node, "Target")?;
                let external = plain_attr(node, "TargetMode")
                    .is_some_and(|mode| mode.eq_ignore_ascii_case("external"));
                let target = if external {
                    target.to_owned()
                } else {
                    resolve(directory, target)
                };
                Some((
                    id.to_owned(),
                    Relationship {
                        kind: kind.to_owned(),
                        target,
                        external,
                    },
                ))
            })
            .collect();
        Relationships { by_id }
    }

    pub fn main_document(&mut self) -> Option<String> {
        let root = self.relationships("");
        if let Some(relationship) = root.of_kind(OFFICE_DOCUMENT) {
            return Some(relationship.target.clone());
        }
        self.names
            .contains_key("word/document.xml")
            .then(|| "word/document.xml".to_owned())
    }
}

fn decode_text(bytes: Vec<u8>) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let little = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

fn resolve(directory: &str, target: &str) -> String {
    let target = target.split('#').next().unwrap_or_default();
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        directory.split('/').filter(|part| !part.is_empty()).collect()
    };
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            segment => parts.push(segment),
        }
    }
    let joined = parts.join("/");
    percent_decode(&joined)
}

fn percent_decode(text: &str) -> String {
    if !text.contains('%') {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Some(byte) = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
