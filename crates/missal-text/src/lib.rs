pub mod fonts;
pub mod input;
pub mod label;
pub mod layout;
pub mod paint;
mod synth;
mod system;

pub use fonts::{FaceId, FontFamily, UI_FAMILY, URDU_FAMILY};
pub use label::{Label, label};
pub use layout::*;
pub use fonts::face;
pub use input::{TextInput, TextInputEvent};
pub use paint::{TextPaint, caret_bounds, paint_caret, paint_text};
pub use system::{TextSystem, init, to_hsla, to_rgba};
