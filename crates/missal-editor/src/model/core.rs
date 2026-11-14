use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;

use missal_core::document::{Align, Block, Document, Inline, ListType, TextFormat};
use missal_core::{FieldCatalog, FieldMarkers, FieldReference};
use missal_page::{Affinity, BlockPath, Edit, Position};

use super::clip::{self, Fragment};
use super::edit::{self, Backward};
use super::history::{Entry, History, Kind};
use super::inline;
use super::lists;
use super::selection::{
    self, Selection, block_len, clamp_selection, document_end, document_start, effective_range,
    end_of, pos, start_of,
};
use super::tree::{self, inlines};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub edit: Edit,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked {
    pub path: BlockPath,
    pub range: Range<usize>,
}

enum Touch {
    Hosts(Vec<BlockPath>),
    Structure,
}

pub struct EditorCore {
    doc: Document,
    selection: Selection,
    history: History,
    pending: Option<TextFormat>,
    marked: Option<Marked>,
    pending_token: Option<BlockPath>,
    catalog: Rc<FieldCatalog>,
    markers: FieldMarkers,
    state: u64,
    next_state: u64,
    saved: u64,
    saved_doc: Document,
    snapshot_state: Cell<u64>,
}

fn caret_of(doc: &Document) -> Selection {
    Selection::caret(document_start(doc).unwrap_or_else(|| pos(BlockPath::top(0), 0)))
}

impl EditorCore {
    pub fn new(document: Document, catalog: Rc<FieldCatalog>, markers: FieldMarkers) -> EditorCore {
        let mut core = EditorCore {
            doc: Document::empty(),
            selection: Selection::caret(pos(BlockPath::top(0), 0)),
            history: History::default(),
            pending: None,
            marked: None,
            pending_token: None,
            catalog,
            markers,
            state: 0,
            next_state: 1,
            saved: 0,
            saved_doc: Document::empty(),
            snapshot_state: Cell::new(0),
        };
        core.load(document);
        core
    }

    pub fn load(&mut self, mut document: Document) {
        edit::ensure_text_block(&mut document);
        self.selection = caret_of(&document);
        self.saved_doc = document.clone();
        self.doc = document;
        self.history.clear();
        self.pending = None;
        self.marked = None;
        self.pending_token = None;
        self.state = self.fresh_state();
        self.saved = self.state;
        self.snapshot_state.set(0);
    }

    pub fn load_dirty(&mut self, document: Document) {
        self.load(document);
        self.saved = 0;
    }

    fn fresh_state(&mut self) -> u64 {
        let state = self.next_state;
        self.next_state += 1;
        state
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn marked(&self) -> Option<&Marked> {
        self.marked.as_ref()
    }

    pub fn catalog(&self) -> &Rc<FieldCatalog> {
        &self.catalog
    }

    pub fn markers(&self) -> &FieldMarkers {
        &self.markers
    }

    pub fn set_fields(&mut self, catalog: Rc<FieldCatalog>, markers: FieldMarkers) {
        self.catalog = catalog;
        self.markers = markers;
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn is_dirty(&self) -> bool {
        self.state != self.saved
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo_depth(&self) -> usize {
        self.history.undo_len()
    }

    pub fn pending_format(&self) -> Option<TextFormat> {
        self.pending
    }

    pub fn snapshot(&self) -> (Document, u64) {
        self.snapshot_state.set(self.state);
        (self.doc.clone(), self.state)
    }

    pub fn mark_saved(&mut self, revision: u64) {
        if revision == self.state {
            self.saved = revision;
            self.saved_doc = self.doc.clone();
            return;
        }
        let mut document = self.doc.clone();
        for entry in self.history.undo_entries().rev() {
            if entry.state_after == revision {
                break;
            }
            let restored: Vec<Block> = entry.blocks.clone();
            document
                .blocks
                .splice(entry.start..entry.start + entry.len, restored);
            if entry.state_before == revision {
                self.saved = revision;
                self.saved_doc = document;
                return;
            }
        }
        if self
            .history
            .undo_entries()
            .any(|e| e.state_after == revision)
        {
            self.saved = revision;
            self.saved_doc = document;
        }
    }

    pub fn discard(&mut self) {
        let document = self.saved_doc.clone();
        let saved = self.saved;
        self.load(document);
        self.state = saved;
        self.saved = saved;
    }

    pub fn break_coalescing(&mut self) {
        self.history.break_coalescing();
    }

    fn transact(
        &mut self,
        range: Range<usize>,
        kind: Kind,
        host: Option<BlockPath>,
        f: impl FnOnce(&mut Document, &FieldCatalog, &FieldMarkers) -> Option<(Selection, Touch)>,
    ) -> Option<Change> {
        let total = self.doc.blocks.len();
        let range = range.start.min(total)..range.end.min(total).max(range.start.min(total));
        let mergeable = self
            .history
            .mergeable(kind, host.as_ref(), &range, &self.selection)
            && self
                .history
                .last()
                .is_some_and(|last| last.state_after != self.snapshot_state.get());
        let snapshot = if mergeable {
            Vec::new()
        } else {
            self.doc.blocks[range.clone()].to_vec()
        };
        let before = self.selection.clone();
        let catalog = self.catalog.clone();
        let markers = self.markers.clone();
        let (selection, touch) = f(&mut self.doc, &catalog, &markers)?;
        let after_len = (range.len() + self.doc.blocks.len()).saturating_sub(total);
        let selection = clamp_selection(&self.doc, &selection);
        let state = self.fresh_state();
        if mergeable {
            self.history.touch(selection.clone(), state);
        } else {
            self.history.push(Entry {
                start: range.start,
                blocks: snapshot,
                len: after_len,
                before,
                after: selection.clone(),
                state_before: self.state,
                state_after: state,
                kind,
                host,
                at: Instant::now(),
            });
        }
        self.state = state;
        self.selection = selection;
        if kind != Kind::Typing && kind != Kind::Compose {
            self.pending = None;
        }
        let edit = match touch {
            Touch::Hosts(paths) if range.len() == 1 && after_len == 1 && !paths.is_empty() => {
                let k = range.start;
                let is_table = matches!(self.doc.blocks.get(k), Some(Block::Table(_)));
                let same_top = paths.iter().all(|p| p.top_index() == k);
                let same_cell = paths
                    .iter()
                    .all(|p| p.steps().len() >= 4 && p.steps()[..3] == paths[0].steps()[..3]);
                if same_top && (!is_table || same_cell) {
                    Edit::Block(paths[0].clone())
                } else {
                    Edit::Splice {
                        start: range.start,
                        removed: range.len(),
                        inserted: after_len,
                    }
                }
            }
            _ => Edit::Splice {
                start: range.start,
                removed: range.len(),
                inserted: after_len,
            },
        };
        Some(Change { edit, kind })
    }

    fn merge_into_last(
        &mut self,
        range: Range<usize>,
        path: BlockPath,
        f: impl FnOnce(&mut Document, &FieldCatalog, &FieldMarkers) -> edit::Converted,
    ) -> Option<Change> {
        let snapshot_state = self.snapshot_state.get();
        let covered = self.history.last().is_some_and(|last| {
            last.start == range.start
                && last.len == range.len()
                && last.state_after != snapshot_state
        });
        let total = self.doc.blocks.len();
        let snapshot = (!covered).then(|| self.doc.blocks[range.clone()].to_vec());
        let catalog = self.catalog.clone();
        let markers = self.markers.clone();
        let converted = f(&mut self.doc, &catalog, &markers);
        if converted.changes.is_empty() {
            return None;
        }
        let map = |p: &Position| -> Position {
            if p.path == path {
                Position {
                    offset: converted.map(p.offset),
                    ..p.clone()
                }
            } else {
                p.clone()
            }
        };
        self.selection = Selection::new(map(&self.selection.anchor), map(&self.selection.head));
        let after_len = (range.len() + self.doc.blocks.len()).saturating_sub(total);
        let state = self.fresh_state();
        self.selection = clamp_selection(&self.doc, &self.selection);
        match snapshot {
            None => {
                if let Some(last) = self.history.last_mut() {
                    last.state_after = state;
                    last.len = after_len;
                }
            }
            Some(blocks) => self.history.push(Entry {
                start: range.start,
                blocks,
                len: after_len,
                before: self.selection.clone(),
                after: self.selection.clone(),
                state_before: self.state,
                state_after: state,
                kind: Kind::Field,
                host: None,
                at: Instant::now(),
            }),
        }
        self.history.break_coalescing();
        self.state = state;
        Some(Change {
            edit: Edit::Splice {
                start: range.start,
                removed: range.len(),
                inserted: after_len,
            },
            kind: Kind::Field,
        })
    }

    pub fn set_selection(&mut self, selection: Selection) -> Option<Change> {
        let selection = clamp_selection(&self.doc, &selection);
        if selection != self.selection {
            self.pending = None;
            self.history.break_coalescing();
        }
        self.selection = selection;
        self.marked = None;
        self.flush_pending_token()
    }

    pub fn set_selection_keep_marked(&mut self, selection: Selection) {
        self.selection = clamp_selection(&self.doc, &selection);
    }

    fn flush_pending_token(&mut self) -> Option<Change> {
        let path = self.pending_token.clone()?;
        if self.marked.is_some() {
            return None;
        }
        let caret = (self.selection.head.path == path).then_some(self.selection.head.offset);
        let found = inlines(&self.doc, &path)?;
        if !edit::has_token(&found, &self.catalog, &self.markers, caret) {
            if !edit::has_token(&found, &self.catalog, &self.markers, None) {
                self.pending_token = None;
            }
            return None;
        }
        drop(found);
        self.pending_token = None;
        let k = path.top_index();
        let target = path.clone();
        self.merge_into_last(k..k + 1, path, move |doc, catalog, markers| {
            edit::convert_tokens(doc, &target, catalog, markers, caret)
        })
    }

    fn note_pending_token(&mut self, path: &BlockPath) {
        if let Some(found) = inlines(&self.doc, path)
            && edit::has_token(&found, &self.catalog, &self.markers, None)
        {
            self.pending_token = Some(path.clone());
        }
    }

    fn range(&self) -> (Position, Position) {
        effective_range(&self.doc, &self.selection)
    }

    pub fn effective_range(&self) -> (Position, Position) {
        self.range()
    }

    fn replace_range_with<F>(&mut self, kind: Kind, host: Option<BlockPath>, f: F) -> Option<Change>
    where
        F: FnOnce(
            &mut Document,
            Position,
            &FieldCatalog,
            &FieldMarkers,
        ) -> Option<(Position, Vec<BlockPath>)>,
    {
        let (s, e) = self.range();
        let collapsed = self.selection.is_collapsed();
        let range = s.path.top_index()..e.path.top_index() + 1;
        let single = s.path == e.path;
        self.transact(range, kind, host, move |doc, catalog, markers| {
            let at = if collapsed {
                s.clone()
            } else {
                edit::delete_range(doc, &s, &e)
            };
            let (caret, mut hosts) = f(doc, at, catalog, markers)?;
            hosts.push(caret.path.clone());
            let touch = if single || collapsed {
                Touch::Hosts(hosts)
            } else {
                Touch::Structure
            };
            Some((Selection::caret(caret), touch))
        })
    }

    pub fn selected_plain_text(&self) -> String {
        let (s, e) = self.range();
        let mut parts = Vec::new();
        for (path, range) in edit::hosts_between(&self.doc, &s, &e) {
            if let Some(found) = inlines(&self.doc, &path) {
                let text: String = inline::text_of(&inline::slice(&found, range))
                    .chars()
                    .filter(|&c| c != missal_page::OBJECT)
                    .collect();
                parts.push(text);
            }
        }
        parts.join("\n")
    }

    pub fn insert_text(&mut self, text: &str) -> Option<Change> {
        let text = inline::strip_objects(&text.replace("\r\n", "\n").replace('\r', "\n"));
        if text.is_empty() {
            return None;
        }
        if text.contains('\n') {
            return self.paste_text(&text);
        }
        let open_first = self.markers.open().chars().next();
        if self.marked.is_none()
            && !self.selection.is_collapsed()
            && text.chars().count() == 1
            && text.chars().next() == open_first
        {
            let selected = self.selected_plain_text();
            let trimmed = missal_core::text::js_trim(&selected).to_owned();
            if !trimmed.is_empty() {
                let reference = self.catalog.reference_for(&trimmed);
                return self.insert_field(reference);
            }
        }
        let pending = self.pending;
        let collapsed = self.selection.is_collapsed();
        let host = collapsed.then(|| self.selection.head.path.clone());
        let change =
            self.replace_range_with(Kind::Typing, host, move |doc, at, catalog, markers| {
                let template = edit::template(doc, &at, pending);
                let caret = edit::insert_text(doc, &at, &text, &template);
                let converted =
                    edit::convert_tokens(doc, &caret.path, catalog, markers, Some(caret.offset));
                let caret = pos(caret.path.clone(), converted.map(caret.offset))
                    .with_affinity(Affinity::Upstream);
                Some((caret, Vec::new()))
            });
        let path = self.selection.head.path.clone();
        self.note_pending_token(&path);
        change
    }

    pub fn insert_field(&mut self, reference: FieldReference) -> Option<Change> {
        self.marked = None;
        self.replace_range_with(Kind::Field, None, move |doc, at, _, _| {
            let template = edit::template(doc, &at, None);
            let caret = edit::insert_inlines(
                doc,
                &at,
                vec![edit::field_inline(reference, Some(&template))],
            );
            Some((caret, Vec::new()))
        })
    }

    pub fn split(&mut self) -> Option<Change> {
        self.marked = None;
        let (s, e) = self.range();
        let range = s.path.top_index()..e.path.top_index() + 1;
        let collapsed = self.selection.is_collapsed();
        self.transact(range, Kind::Structure, None, move |doc, _, _| {
            let at = if collapsed {
                s.clone()
            } else {
                edit::delete_range(doc, &s, &e)
            };
            let caret = edit::split(doc, &at);
            Some((Selection::caret(caret), Touch::Structure))
        })
    }

    pub fn line_break(&mut self) -> Option<Change> {
        self.marked = None;
        self.replace_range_with(Kind::Structure, None, |doc, at, _, _| {
            Some((edit::line_break(doc, &at), Vec::new()))
        })
    }

    pub fn delete(&mut self, forward: bool, word: bool) -> Option<Change> {
        self.marked = None;
        if !self.selection.is_collapsed() {
            let (s, e) = self.range();
            let range = s.path.top_index()..e.path.top_index() + 1;
            let single = s.path == e.path;
            return self.transact(range, Kind::Structure, None, move |doc, _, _| {
                let caret = edit::delete_range(doc, &s, &e);
                let touch = if single {
                    Touch::Hosts(vec![caret.path.clone()])
                } else {
                    Touch::Structure
                };
                Some((Selection::caret(caret), touch))
            });
        }
        let at = self.selection.head.clone();
        let plan = if forward {
            edit::forward(&self.doc, &at, word)
        } else {
            edit::backward(&self.doc, &at, word)
        };
        let k = at.path.top_index();
        match plan {
            Backward::Nothing => None,
            Backward::Move(position) => self.set_selection(Selection::caret(position)),
            Backward::Range(s, e) => {
                if s.offset == e.offset {
                    return None;
                }
                let host = Some(at.path.clone());
                self.transact(k..k + 1, Kind::Delete, host, move |doc, _, _| {
                    let caret = edit::delete_range(doc, &s, &e);
                    Some((
                        Selection::caret(caret.clone()),
                        Touch::Hosts(vec![caret.path]),
                    ))
                })
            }
            Backward::Merge { into, from } => {
                let lo = into.top_index().min(from.top_index());
                let hi = into.top_index().max(from.top_index());
                let keep = forward;
                self.transact(lo..hi + 1, Kind::Structure, None, move |doc, _, _| {
                    let caret = edit::merge_hosts(doc, &into, &from);
                    let caret = if keep {
                        pos(caret.path, caret.offset)
                    } else {
                        caret
                    };
                    Some((Selection::caret(caret), Touch::Structure))
                })
            }
            Backward::RemoveBlock(path) => {
                let lo = path.top_index().min(k);
                let hi = path.top_index().max(k);
                let removed_before = path < at.path && path.steps().len() == at.path.steps().len();
                self.transact(lo..hi + 1, Kind::Structure, None, move |doc, _, _| {
                    let (prefix, index) = tree::split_block_path(&path);
                    let prefix = prefix.to_vec();
                    tree::container_mut(doc, &prefix)?.remove(index);
                    let mut caret = at.clone();
                    if removed_before && caret.path.steps()[..prefix.len()] == prefix[..] {
                        let mut steps = caret.path.steps().to_vec();
                        steps[prefix.len()] -= 1;
                        caret.path = BlockPath(steps);
                    }
                    Some((Selection::caret(caret), Touch::Structure))
                })
            }
            Backward::Outdent(path) => {
                self.transact(k..k + 1, Kind::Structure, None, move |doc, _, _| {
                    let caret = lists::outdent(doc, &path);
                    Some((Selection::caret(caret), Touch::Structure))
                })
            }
            Backward::ToParagraph(path) => {
                self.transact(k..k + 1, Kind::Structure, None, move |doc, _, _| {
                    let caret = edit::to_paragraph(doc, &path);
                    Some((Selection::caret(caret), Touch::Structure))
                })
            }
        }
    }

    pub fn flag_state(&self, flag: TextFormat) -> bool {
        if self.selection.is_collapsed() {
            if let Some(pending) = self.pending {
                return pending.contains(flag);
            }
            let template = edit::template(&self.doc, &self.selection.head, None);
            return inline::has_flag(&template, flag);
        }
        let (s, e) = self.range();
        edit::flag_state(&self.doc, &s, &e, flag).unwrap_or(false)
    }

    pub fn toggle_flag(&mut self, flag: TextFormat) -> Option<Change> {
        if self.selection.is_collapsed() {
            let template = edit::template(&self.doc, &self.selection.head, self.pending);
            let mut format = template.format();
            let on = !inline::has_flag(&template, flag);
            format.set(flag, on);
            self.pending = Some(format);
            self.history.break_coalescing();
            return None;
        }
        let (s, e) = self.range();
        let on = !edit::flag_state(&self.doc, &s, &e, flag).unwrap_or(false);
        let range = s.path.top_index()..e.path.top_index() + 1;
        let selection = self.selection.clone();
        let hosts: Vec<BlockPath> = edit::hosts_between(&self.doc, &s, &e)
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        self.transact(range, Kind::Format, None, move |doc, _, _| {
            edit::set_flag(doc, &s, &e, flag, on);
            Some((selection, Touch::Hosts(hosts)))
        })
    }

    pub fn align(&self) -> Align {
        edit::effective_align(&self.doc, &self.selection.head.path)
    }

    pub fn set_align(&mut self, align: Align) -> Option<Change> {
        let (s, e) = self.range();
        let range = s.path.top_index()..e.path.top_index() + 1;
        let selection = self.selection.clone();
        self.transact(range, Kind::Format, None, move |doc, _, _| {
            let touched = edit::set_align(doc, &s, &e, align);
            if touched.is_empty() {
                return None;
            }
            Some((selection, Touch::Hosts(touched)))
        })
    }

    pub fn list_state(&self) -> Option<ListType> {
        lists::list_state(&self.doc, &self.selection.head.path)
    }

    pub fn toggle_list(&mut self, kind: ListType) -> Option<Change> {
        let (s, e) = self.selection.ordered();
        let total = self.doc.blocks.len();
        let lo = s.path.top_index();
        let hi = e.path.top_index();
        let range = lo..(hi + 1).min(total);
        let reversed = self.selection.is_reversed();
        self.transact(range, Kind::Structure, None, move |doc, _, _| {
            let (a, b) = lists::toggle(doc, &s, &e, kind)?;
            let selection = if reversed {
                Selection::new(b, a)
            } else {
                Selection::new(a, b)
            };
            Some((selection, Touch::Structure))
        })
    }

    pub fn indent(&mut self, outward: bool) -> Option<Change> {
        let path = self.selection.head.path.clone();
        if tree::host_kind(&self.doc, &path) != Some(tree::HostKind::Item) {
            return None;
        }
        let k = path.top_index();
        let offset = self.selection.head.offset;
        self.transact(k..k + 1, Kind::Structure, None, move |doc, _, _| {
            let caret = lists::indent_or_outdent(doc, &path, outward)?;
            Some((Selection::caret(pos(caret.path, offset)), Touch::Structure))
        })
    }

    pub fn copy(&self) -> Option<Fragment> {
        if self.selection.is_collapsed() {
            return None;
        }
        let (s, e) = self.range();
        let fragment = clip::copy(&self.doc, &s, &e);
        (!fragment.is_empty()).then_some(fragment)
    }

    pub fn paste_fragment(&mut self, fragment: Fragment) -> Option<Change> {
        if fragment.is_empty() {
            return None;
        }
        self.marked = None;
        let (s, e) = self.range();
        let collapsed = self.selection.is_collapsed();
        let range = s.path.top_index()..e.path.top_index() + 1;
        self.transact(range, Kind::Paste, None, move |doc, catalog, markers| {
            let at = if collapsed {
                s.clone()
            } else {
                edit::delete_range(doc, &s, &e)
            };
            let first_top = at.path.top_index();
            let before_total = doc.blocks.len();
            let caret = clip::paste(doc, &at, fragment);
            let span = doc.blocks.len() + 1 - before_total;
            let caret_path = caret.path.clone();
            let mut caret_offset = caret.offset;
            for k in first_top..(first_top + span).min(doc.blocks.len()) {
                for path in tree::text_paths_of_block(&doc.blocks[k], BlockPath::top(k)) {
                    let exclude = (path == caret_path).then_some(caret_offset);
                    let converted = edit::convert_tokens(doc, &path, catalog, markers, exclude);
                    if path == caret_path {
                        caret_offset = converted.map(caret_offset);
                    }
                }
            }
            Some((
                Selection::caret(pos(caret_path, caret_offset)),
                Touch::Structure,
            ))
        })
    }

    pub fn paste_text(&mut self, text: &str) -> Option<Change> {
        let (s, _) = self.range();
        let template = edit::template(&self.doc, &s, self.pending);
        let text = if clip::looks_like_html(text) {
            clip::html_to_text(text)
        } else {
            text.to_owned()
        };
        let fragment = clip::from_text(&text, &template);
        self.paste_fragment(fragment)
    }

    pub fn compose(&mut self, text: &str, selected: Option<Range<usize>>) -> Option<Change> {
        let text = inline::strip_objects(&text.replace(['\r', '\n'], " "));
        let pending = self.pending;
        let marked = self.marked.clone();
        let (s, e) = match &marked {
            Some(m) => (
                pos(m.path.clone(), m.range.start),
                pos(m.path.clone(), m.range.end),
            ),
            None => self.range(),
        };
        let collapsed = s.path == e.path && s.offset == e.offset;
        if collapsed && text.is_empty() {
            self.marked = None;
            return None;
        }
        let host = Some(s.path.clone());
        let range = s.path.top_index()..e.path.top_index() + 1;
        let replaced = s.clone();
        let inserted = text.len();
        let change = self.transact(range, Kind::Compose, host, move |doc, _, _| {
            let at = if collapsed {
                s.clone()
            } else {
                edit::delete_range(doc, &s, &e)
            };
            let template = edit::template(doc, &at, pending);
            let end = edit::insert_text(doc, &at, &text, &template);
            let caret = match selected {
                Some(r) => {
                    let base = at.offset;
                    let a = snap_in(&text, r.start);
                    let b = snap_in(&text, r.end);
                    Selection::new(
                        pos(at.path.clone(), base + a),
                        pos(at.path.clone(), base + b),
                    )
                }
                None => Selection::caret(end.clone()),
            };
            Some((caret, Touch::Hosts(vec![at.path.clone()])))
        });
        self.marked = (inserted > 0).then(|| Marked {
            path: replaced.path.clone(),
            range: replaced.offset..replaced.offset + inserted,
        });
        if inserted == 0 && change.is_none() && marked.is_some() {
            self.selection = Selection::caret(replaced);
        }
        change
    }

    pub fn set_marked(&mut self, marked: Option<Marked>) {
        self.marked = marked;
    }

    pub fn commit(&mut self, text: &str) -> Option<Change> {
        let marked = self.marked.take();
        let Some(marked) = marked else {
            return self.insert_text(text);
        };
        let text = inline::strip_objects(&text.replace(['\r', '\n'], " "));
        let k = marked.path.top_index();
        let pending = self.pending;
        let host = Some(marked.path.clone());
        let change = self.transact(
            k..k + 1,
            Kind::Compose,
            host,
            move |doc, catalog, markers| {
                let s = pos(marked.path.clone(), marked.range.start);
                let e = pos(marked.path.clone(), marked.range.end);
                let at = edit::delete_range(doc, &s, &e);
                let template = edit::template(doc, &at, pending);
                let caret = if text.is_empty() {
                    at
                } else {
                    edit::insert_text(doc, &at, &text, &template)
                };
                let converted =
                    edit::convert_tokens(doc, &caret.path, catalog, markers, Some(caret.offset));
                let caret = pos(caret.path.clone(), converted.map(caret.offset))
                    .with_affinity(Affinity::Upstream);
                Some((
                    Selection::caret(caret.clone()),
                    Touch::Hosts(vec![caret.path]),
                ))
            },
        );
        self.history.break_coalescing();
        let path = self.selection.head.path.clone();
        self.note_pending_token(&path);
        change
    }

    pub fn unmark(&mut self) -> Option<Change> {
        let marked = self.marked.take()?;
        self.history.break_coalescing();
        let k = marked.path.top_index();
        let caret = (self.selection.head.path == marked.path).then_some(self.selection.head.offset);
        let path = marked.path.clone();
        let target = path.clone();
        let change = self.merge_into_last(k..k + 1, path, move |doc, catalog, markers| {
            edit::convert_tokens(doc, &target, catalog, markers, caret)
        });
        if change.is_none() {
            let path = self.selection.head.path.clone();
            self.note_pending_token(&path);
        }
        change
    }

    pub fn reveal_field(&mut self, path: &BlockPath, offset: usize) -> Option<Change> {
        let k = path.top_index();
        let path = path.clone();
        let change = self.transact(k..k + 1, Kind::Field, None, move |doc, catalog, markers| {
            let (a, b) = edit::reveal_field(doc, &path, offset, catalog, markers)?;
            Some((Selection::new(a, b), Touch::Hosts(vec![path.clone()])))
        });
        if change.is_some() {
            self.pending_token = Some(self.selection.head.path.clone());
        }
        change
    }

    pub fn field_at(&self, path: &BlockPath, offset: usize) -> Option<FieldReference> {
        let found = inlines(&self.doc, path)?;
        inline::field_at(&found, offset).map(|f| f.reference.clone())
    }

    pub fn undo(&mut self) -> Option<Change> {
        self.marked = None;
        self.pending = None;
        self.pending_token = None;
        let entry = self.history.pop_undo()?;
        let current: Vec<Block> = self
            .doc
            .blocks
            .splice(entry.start..entry.start + entry.len, entry.blocks.clone())
            .collect();
        let edit = Edit::Splice {
            start: entry.start,
            removed: entry.len,
            inserted: entry.blocks.len(),
        };
        self.state = entry.state_before;
        self.selection = clamp_selection(&self.doc, &entry.before);
        self.history.push_redo(Entry {
            blocks: current,
            len: entry.blocks.len(),
            ..entry
        });
        Some(Change {
            edit,
            kind: Kind::Structure,
        })
    }

    pub fn redo(&mut self) -> Option<Change> {
        self.marked = None;
        self.pending = None;
        self.pending_token = None;
        let entry = self.history.pop_redo()?;
        let current: Vec<Block> = self
            .doc
            .blocks
            .splice(entry.start..entry.start + entry.len, entry.blocks.clone())
            .collect();
        let edit = Edit::Splice {
            start: entry.start,
            removed: entry.len,
            inserted: entry.blocks.len(),
        };
        self.state = entry.state_after;
        self.selection = clamp_selection(&self.doc, &entry.after);
        self.history.push_undo_keep_redo(Entry {
            blocks: current,
            len: entry.blocks.len(),
            ..entry
        });
        Some(Change {
            edit,
            kind: Kind::Structure,
        })
    }

    pub fn select_all(&mut self) -> Option<Change> {
        if let Some(cell) = tree::cell_of(&self.doc, &self.selection.head.path)
            && let Some(blocks) = tree::container(&self.doc, &cell)
        {
            let paths = tree::text_paths_in(blocks, &cell);
            if let (Some(first), Some(last)) = (paths.first(), paths.last()) {
                let whole =
                    Selection::new(start_of(first.clone()), end_of(&self.doc, last.clone()));
                let (s, e) = self.selection.ordered();
                let current = Selection::new(s, e);
                if !same_span(&current, &whole) {
                    return self.set_selection(whole);
                }
            }
        }
        let start = document_start(&self.doc)?;
        let end = document_end(&self.doc)?;
        self.set_selection(Selection::new(start, end))
    }

    pub fn cell_step(&self, back: bool) -> Option<Selection> {
        let head = &self.selection.head.path;
        let cell = tree::cell_of(&self.doc, head)?;
        let table_path = &cell[..cell.len() - 2];
        let table = tree::table_at(&self.doc, table_path)?;
        let cells = tree::cells_of_table(table, &BlockPath::new(table_path));
        let index = cells.iter().position(|c| c.steps() == cell.as_slice())?;
        let candidates: Box<dyn Iterator<Item = &BlockPath>> = if back {
            Box::new(cells[..index].iter().rev())
        } else {
            Box::new(cells[index + 1..].iter())
        };
        for candidate in candidates {
            if let Some(blocks) = tree::container(&self.doc, candidate.steps())
                && let Some(first) = tree::text_paths_in(blocks, candidate.steps())
                    .into_iter()
                    .next()
            {
                return Some(Selection::caret(start_of(first)));
            }
        }
        None
    }

    pub fn in_table(&self) -> bool {
        tree::cell_of(&self.doc, &self.selection.head.path).is_some()
    }

    pub fn in_list(&self) -> bool {
        tree::host_kind(&self.doc, &self.selection.head.path) == Some(tree::HostKind::Item)
    }

    pub fn block_text(&self, path: &BlockPath) -> Option<String> {
        inlines(&self.doc, path).map(|found| inline::text_of(&found))
    }

    pub fn block_len(&self, path: &BlockPath) -> Option<usize> {
        block_len(&self.doc, path)
    }

    pub fn is_valid(&self, position: &Position) -> bool {
        selection::is_valid(&self.doc, position)
    }

    pub fn field_count(&self) -> usize {
        let mut count = 0;
        let mut doc = self.doc.clone();
        doc.for_each_field_mut(&mut |_| count += 1);
        count
    }

    pub fn inlines_at(&self, path: &BlockPath) -> Option<Vec<Inline>> {
        inlines(&self.doc, path).map(|c| c.into_owned())
    }
}

fn same_span(a: &Selection, b: &Selection) -> bool {
    let (a1, a2) = a.ordered();
    let (b1, b2) = b.ordered();
    a1.path == b1.path && a1.offset == b1.offset && a2.path == b2.path && a2.offset == b2.offset
}

fn snap_in(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}
