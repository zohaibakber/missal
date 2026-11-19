pub mod model;
pub mod view;

pub use model::{
    BlockCommand, Change, EditorCore, Fragment, Kind, Marked, Match, Query, Selection, TextKind,
};
pub use view::{Editor, EditorEvent, EditorFields, EditorTheme, init};
