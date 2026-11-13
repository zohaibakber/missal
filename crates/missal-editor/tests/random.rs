mod common;

use common::*;
use missal_core::document::{Align, Block, ListType, TextFormat};
use missal_core::{Document, FieldMarkers};
use missal_editor::model::selection::pos;
use missal_editor::model::tree::all_text_paths;
use missal_editor::{EditorCore, Selection};
use missal_page::{DocSource, PageEngine, TokenFields};

const SNIPPETS: &[&str] = &[
    "ک",
    "ی",
    "ا",
    " ",
    "سلام",
    "PPC 380",
    "۱۲",
    "«",
    "»",
    "«جرم»",
    "«بینک»",
    "«نامعلوم»",
    "e\u{301}",
    "بَ",
    "👍",
];

fn random_position(core: &EditorCore, rng: &mut Rng) -> missal_page::Position {
    let paths = all_text_paths(core.document());
    let path = paths[rng.below(paths.len())].clone();
    let text = core.block_text(&path).unwrap();
    let mut stops: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    stops.push(text.len());
    let offset = stops[rng.below(stops.len())];
    let position = pos(path, offset);
    if core.is_valid(&position) {
        position
    } else {
        pos(position.path, 0)
    }
}

fn check(core: &EditorCore, step: usize, op: &str) {
    let encoded = core.document().encode();
    let decoded = Document::decode(&encoded)
        .unwrap_or_else(|e| panic!("step {step} {op}: decode failed {e:?}"));
    assert_eq!(
        decoded.encode(),
        encoded,
        "step {step} {op}: codec round trip"
    );
    let selection = core.selection();
    assert!(
        core.is_valid(&selection.anchor),
        "step {step} {op}: anchor {:?}",
        selection.anchor
    );
    assert!(
        core.is_valid(&selection.head),
        "step {step} {op}: head {:?}",
        selection.head
    );
    if let Some(marked) = core.marked() {
        let length = core.block_len(&marked.path).expect("marked block exists");
        assert!(marked.range.end <= length, "step {step} {op}: marked range");
    }
}

fn random_op(
    core: &mut EditorCore,
    rng: &mut Rng,
    fragments: &mut Vec<missal_editor::Fragment>,
) -> (&'static str, Option<missal_editor::Change>) {
    match rng.below(26) {
        0..=5 => {
            let snippet = SNIPPETS[rng.below(SNIPPETS.len())];
            ("type", core.insert_text(snippet))
        }
        6 => ("backspace", core.delete(false, rng.chance(25))),
        7 => ("delete", core.delete(true, rng.chance(25))),
        8 => ("enter", core.split()),
        9 => ("line-break", core.line_break()),
        10 => {
            let position = random_position(core, rng);
            ("caret", core.set_selection(Selection::caret(position)))
        }
        11 => {
            let a = random_position(core, rng);
            let b = random_position(core, rng);
            ("select", core.set_selection(Selection::new(a, b)))
        }
        12 => {
            let flag = [TextFormat::BOLD, TextFormat::ITALIC, TextFormat::UNDERLINE][rng.below(3)];
            ("format", core.toggle_flag(flag))
        }
        13 => {
            let align = [Align::Left, Align::Center, Align::Right, Align::Justify][rng.below(4)];
            ("align", core.set_align(align))
        }
        14 => {
            let kind = if rng.chance(50) {
                ListType::Bullet
            } else {
                ListType::Number
            };
            ("list", core.toggle_list(kind))
        }
        15 => ("indent", core.indent(rng.chance(50))),
        16 => (
            "field",
            core.insert_field(if rng.chance(50) { fir_no() } else { offence() }),
        ),
        17 => {
            let head = core.selection().head.clone();
            if core.field_at(&head.path, head.offset).is_some() {
                ("reveal", core.reveal_field(&head.path, head.offset))
            } else {
                ("reveal-miss", None)
            }
        }
        18 => {
            if let Some(fragment) = core.copy() {
                fragments.push(fragment);
            }
            ("copy", None)
        }
        19 => match fragments.get(rng.below(fragments.len().max(1))).cloned() {
            Some(fragment) => ("paste", core.paste_fragment(fragment)),
            None => ("paste-none", None),
        },
        20 => ("paste-text", core.paste_text("ایک\nدو «جرم»\n\nتین")),
        21 => {
            core.compose("ک", None);
            core.compose("کی", Some(2..2));
            if rng.chance(70) {
                ("compose-commit", core.commit("کیا"))
            } else {
                ("compose-cancel", core.compose("", None))
            }
        }
        22 => match core.cell_step(rng.chance(50)) {
            Some(selection) => ("cell-step", core.set_selection(selection)),
            None => ("cell-step-none", None),
        },
        23 => ("undo", core.undo()),
        24 => ("redo", core.redo()),
        _ => ("select-all", core.select_all()),
    }
}

fn episode(document: Document, seed: u64, steps: usize, relayout: bool) {
    let mut rng = Rng(seed);
    let mut core = EditorCore::new(document, catalog(), FieldMarkers::default());
    let original = core.document().encode();
    let fields = TokenFields;
    let mut engine = PageEngine::new();
    let mut packet = relayout.then(|| engine.layout(&[DocSource::new(core.document(), &fields)]));
    let mut fragments = Vec::new();
    let mut changed = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for step in 0..steps {
        let (op, change) = random_op(&mut core, &mut rng, &mut fragments);
        check(&core, step, op);
        if change.is_some() {
            changed += 1;
            kinds.insert(op);
        }
        if let (Some(packet), Some(change)) = (packet.as_mut(), change) {
            engine.update(
                packet,
                0,
                &DocSource::new(core.document(), &fields),
                &change.edit,
            );
            assert!(
                packet.caret(&core.selection().head).is_some(),
                "seed {seed} step {step} {op}: caret has geometry"
            );
        }
    }
    assert!(
        changed * 3 > steps,
        "seed {seed}: only {changed} of {steps} operations changed anything"
    );
    assert!(
        kinds.len() >= 8,
        "seed {seed}: too few kinds of change {kinds:?}"
    );
    if let Some(packet) = &packet {
        let fresh = PageEngine::new().layout(&[DocSource::new(core.document(), &fields)]);
        assert_eq!(
            packet.page_count(),
            fresh.page_count(),
            "seed {seed}: page count"
        );
        let lines = |p: &missal_page::Packet| -> Vec<_> {
            p.lines(0)
                .into_iter()
                .map(|l| (l.path, l.page, l.text))
                .collect()
        };
        assert_eq!(
            lines(packet),
            lines(&fresh),
            "seed {seed}: incremental layout equals fresh layout"
        );
    }
    let finished = core.document().encode();
    let mut undone = 0;
    while core.undo().is_some() {
        undone += 1;
        check(&core, steps + undone, "undo-all");
    }
    if core.undo_depth() == 0 && undone < 100 {
        assert_eq!(
            core.document().encode(),
            original,
            "seed {seed}: undo of everything restores the original bytes"
        );
    }
    while core.redo().is_some() {
        check(&core, steps, "redo-all");
    }
    assert_eq!(
        core.document().encode(),
        finished,
        "seed {seed}: redo of everything restores the final bytes"
    );
}

fn mixed() -> Document {
    doc(vec![
        p("پہلا پیراگراف PPC 380 کے تحت"),
        para(vec![text("نمبر "), field(fir_no()), text(" مورخہ")]),
        grid(2, 3),
        Block::PageBreak(Default::default()),
        p(""),
        p("آخری"),
    ])
}

#[test]
fn random_editing_keeps_invariants_on_a_mixed_document() {
    for seed in 1..=60u64 {
        episode(mixed(), seed * 7919, 90, seed % 4 == 0);
    }
}

#[test]
fn random_editing_keeps_invariants_on_a_bundled_template() {
    let (_, template) = largest_template();
    let mut head = template.clone();
    head.blocks.truncate(40);
    for seed in 1..=12u64 {
        episode(head.clone(), seed * 104_729, 80, seed % 3 == 0);
    }
}

#[test]
fn long_random_session_on_a_small_page() {
    episode(small_page(mixed()), 0x5eed_cafe, 600, true);
}
