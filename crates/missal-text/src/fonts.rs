use std::sync::{Arc, Mutex, OnceLock};

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

struct Shared {
    collection: Mutex<Collection>,
    blob_ids: [u64; FACES.len()],
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut blob_ids = [0; FACES.len()];
        for (id, face) in blob_ids.iter_mut().zip(FACES.iter()) {
            let blob = Blob::new(Arc::new(face.data));
            *id = blob.id();
            collection.register_fonts(blob, None);
        }
        let urdu = collection.family_id(URDU_FAMILY);
        let ui = collection.family_id(UI_FAMILY);
        let mut fallback = |script: &[u8; 4], family: Option<FamilyId>| {
            if let Some(family) = family {
                collection.append_fallbacks(
                    FallbackKey::new(Script::from_bytes(*script), None),
                    std::iter::once(family),
                );
            }
        };
        for script in [b"Arab", b"Zyyy", b"Zinh"] {
            fallback(script, urdu);
        }
        fallback(b"Latn", ui);
        fallback(b"Zyyy", ui);
        Shared {
            collection: Mutex::new(collection),
            blob_ids,
        }
    })
}

pub(crate) fn face_for_blob(blob_id: u64) -> Option<FaceId> {
    shared()
        .blob_ids
        .iter()
        .position(|&id| id == blob_id)
        .map(|index| FaceId(index as u16))
}

pub(crate) fn font_context() -> FontContext {
    let collection = shared()
        .collection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    FontContext {
        collection,
        source_cache: SourceCache::default(),
    }
}
