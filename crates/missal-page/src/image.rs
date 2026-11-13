use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Other,
}

#[derive(Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct ImageData {
    pub bytes: Arc<[u8]>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    decoded: OnceLock<Option<Arc<DecodedImage>>>,
}

impl PartialEq for ImageData {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes
    }
}

fn base64_value(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => (c - b'A') as u32,
        b'a'..=b'z' => (c - b'a' + 26) as u32,
        b'0'..=b'9' => (c - b'0' + 52) as u32,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    })
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        buffer = (buffer << 6) | base64_value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

pub(crate) fn jpeg_info(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            let components = bytes[i + 9];
            return Some((w, h, components));
        }
        i += 2 + length;
    }
    None
}

impl ImageData {
    pub fn from_bytes(bytes: Vec<u8>) -> Option<Arc<ImageData>> {
        let (format, (width, height)) = if let Some(size) = png_size(&bytes) {
            (ImageFormat::Png, size)
        } else if let Some((w, h, _)) = jpeg_info(&bytes) {
            (ImageFormat::Jpeg, (w, h))
        } else {
            return None;
        };
        Some(Arc::new(ImageData {
            bytes: Arc::from(bytes),
            format,
            width,
            height,
            decoded: OnceLock::new(),
        }))
    }

    pub fn from_data_uri(uri: &str) -> Option<Arc<ImageData>> {
        let rest = uri.strip_prefix("data:")?;
        let (meta, payload) = rest.split_once(',')?;
        if !meta.ends_with(";base64") {
            return None;
        }
        ImageData::from_bytes(decode_base64(payload)?)
    }

    pub fn decoded(&self) -> Option<Arc<DecodedImage>> {
        self.decoded
            .get_or_init(|| {
                let image = match self.format {
                    ImageFormat::Png => {
                        image::load_from_memory_with_format(&self.bytes, image::ImageFormat::Png)
                            .ok()?
                    }
                    ImageFormat::Jpeg => {
                        image::load_from_memory_with_format(&self.bytes, image::ImageFormat::Jpeg)
                            .ok()?
                    }
                    ImageFormat::Other => return None,
                };
                let rgba = image.to_rgba8();
                Some(Arc::new(DecodedImage {
                    width: rgba.width(),
                    height: rgba.height(),
                    rgba: rgba.into_raw(),
                }))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8gd29ybGQ").unwrap(), b"hello world");
    }
}
