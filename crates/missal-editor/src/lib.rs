pub mod model;
pub mod view;

pub use model::{Change, EditorCore, Fragment, Kind, Marked, Selection};
pub use view::{Editor, EditorEvent, EditorFields, EditorTheme, init};
