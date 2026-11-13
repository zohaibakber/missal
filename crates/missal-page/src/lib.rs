mod css;
mod display;
mod engine;
mod fonts;
mod geom;
mod image;
mod mapping;
mod measure;
mod paginate;
mod position;
mod style;
mod table;
mod text;

pub mod paint;
pub mod pdf;

use missal_core::{FieldDisplayText, FieldPresentation, FieldReference};

pub use display::{DisplayItem, DisplayList, FieldMark, Glyph, GlyphRun, RectKind, TextLine};
pub use engine::{DocSource, Edit, LayoutOptions, Packet, Page, PageEngine, RelayoutStats};
pub use geom::{PX_PER_MM, Point, Rect, Size, mm_to_px};
pub use image::{ImageData, ImageFormat};
pub use mapping::{Caret, LineHit, PlacedCell, PlacedLine};
pub use position::{BlockPath, Position, VerticalDirection};
pub use style::SUPPORTED_PROPERTIES;
pub use text::{FieldSpan, InlineLocation, LineBox, OBJECT, OBJECT_LEN, OffsetMap, TextBox, locate, logical_len, logical_text};

pub use missal_text::{Affinity, HorizontalDirection};

pub trait FieldSource {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText;
}

impl FieldSource for FieldPresentation<'_> {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        self.display_text(reference)
    }
}

impl<F> FieldSource for F
where
    F: Fn(&FieldReference) -> FieldDisplayText,
{
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        self(reference)
    }
}

pub struct TokenFields;

impl FieldSource for TokenFields {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        match reference {
            FieldReference::Unresolved(text) => FieldDisplayText {
                text: text.clone(),
                unresolved: true,
            },
            FieldReference::Catalog(id) => FieldDisplayText {
                text: id.to_string(),
                unresolved: true,
            },
            FieldReference::Fir(field) => FieldDisplayText {
                text: missal_core::field::fir_field_label(*field).to_owned(),
                unresolved: false,
            },
        }
    }
}
