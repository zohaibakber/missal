use std::{collections::HashMap, sync::Arc};

use parley::{
    FontContext,
    fontique::{Blob, Collection, CollectionOptions, FallbackKey, FamilyId, Script, SourceCache},
};

pub const URDU_FAMILY: &str = "Missal Jameel Noori Nastaleeq";
pub const UI_FAMILY: &str = "Missal Inter";
pub const URDU_FAMILY_ALIASES: [&str; 2] = [URDU_FAMILY, "Jameel Noori Nastaleeq"];
pub const UI_FAMILY_ALIASES: [&str; 3] = [UI_FAMILY, "Inter", "Inter Variable"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub u16);

#[derive(Debug)]
pub struct Face {
    pub family: &'static str,
    pub weight: u16,
    pub italic: bool,
    pub file_name: &'static str,
    pub data: &'static [u8],
}

macro_rules! face {
    ($family:expr, $weight:expr, $italic:expr, $file:literal) => {
        Face {
            family: $family,
            weight: $weight,
            italic: $italic,
            file_name: $file,
            data: include_bytes!(concat!("../../../assets/fonts/", $file)),
        }
    };
}

static FACES: [Face; 7] = [
    face!(URDU_FAMILY, 400, false, "JameelNooriNastaleeq.ttf"),
    face!(UI_FAMILY, 400, false, "Inter-Regular.ttf"),
    face!(UI_FAMILY, 500, false, "Inter-Medium.ttf"),
    face!(UI_FAMILY, 600, false, "Inter-SemiBold.ttf"),
    face!(UI_FAMILY, 700, false, "Inter-Bold.ttf"),
    face!(UI_FAMILY, 400, true, "Inter-Italic.ttf"),
    face!(UI_FAMILY, 700, true, "Inter-BoldItalic.ttf"),
];

pub fn bundled_faces() -> &'static [Face] {
    &FACES
}

pub fn face(id: FaceId) -> &'static Face {
    &FACES[id.0 as usize]
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    #[default]
    Interface,
    Document,
    Named(Arc<str>),
}

impl FontFamily {
    pub fn named(name: impl Into<Arc<str>>) -> Self {
        Self::Named(name.into())
    }

    pub(crate) fn stack(&self) -> Vec<&str> {
        match self {
            Self::Interface => vec![UI_FAMILY, URDU_FAMILY],
            Self::Document => vec![URDU_FAMILY, UI_FAMILY],
            Self::Named(name) => vec![name, URDU_FAMILY, UI_FAMILY],
        }
    }
}

pub(crate) struct FontRegistry {
    pub(crate) fcx: FontContext,
    faces_by_blob: HashMap<u64, FaceId>,
}

impl FontRegistry {
    pub(crate) fn new() -> Self {
        let mut fcx = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: SourceCache::default(),
        };
        let mut faces_by_blob = HashMap::new();
        for (index, face) in FACES.iter().enumerate() {
            let blob = Blob::new(Arc::new(face.data));
            faces_by_blob.insert(blob.id(), FaceId(index as u16));
            fcx.collection.register_fonts(blob, None);
        }
        let urdu = fcx.collection.family_id(URDU_FAMILY);
        let ui = fcx.collection.family_id(UI_FAMILY);
        let fallback = |collection: &mut Collection, script: &[u8; 4], family: Option<FamilyId>| {
            if let Some(family) = family {
                collection.append_fallbacks(
                    FallbackKey::new(Script::from_bytes(*script), None),
                    std::iter::once(family),
                );
            }
        };
        for script in [b"Arab", b"Zyyy", b"Zinh"] {
            fallback(&mut fcx.collection, script, urdu);
        }
        fallback(&mut fcx.collection, b"Latn", ui);
        fallback(&mut fcx.collection, b"Zyyy", ui);
        Self { fcx, faces_by_blob }
    }

    pub(crate) fn faces_by_blob(&self) -> &HashMap<u64, FaceId> {
        &self.faces_by_blob
    }
}
