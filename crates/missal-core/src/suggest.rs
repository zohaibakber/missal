use std::collections::HashMap;
use std::ops::Range;

use crate::fir::FirProperty;
use crate::ids::PlaceholderId;
use crate::text::{Folded, collapse_whitespace, fold_text};

pub const SUGGESTION_LIMIT: usize = 8;
const MAX_VALUE_CHARS: usize = 160;
const RECENCY_HALF_LIFE: f32 = 24.0;
const RECENCY_WEIGHT: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionKey {
    Fir(FirProperty),
    Placeholder(PlaceholderId),
}

impl SuggestionKey {
    pub const FIR_PROPERTIES: [FirProperty; 7] = [
        FirProperty::Complainant,
        FirProperty::Offence,
        FirProperty::Accused,
        FirProperty::Witness,
        FirProperty::InvestigationOfficer,
        FirProperty::Writer,
        FirProperty::Recovery,
    ];

    pub fn for_property(property: FirProperty) -> Option<SuggestionKey> {
        Self::FIR_PROPERTIES
            .contains(&property)
            .then_some(SuggestionKey::Fir(property))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionEntry {
    pub key: SuggestionKey,
    pub value: String,
    pub stamp: u64,
}

impl SuggestionEntry {
    pub fn new(key: SuggestionKey, value: impl Into<String>, stamp: u64) -> SuggestionEntry {
        SuggestionEntry {
            key,
            value: value.into(),
            stamp,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub value: String,
    pub uses: u32,
    pub highlights: Vec<Range<usize>>,
}

#[derive(Debug, Clone)]
struct Candidate {
    value: Box<str>,
    folded: Box<str>,
    uses: u32,
    stamp: u64,
    score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Prefix,
    Words,
    Inside,
}

type Hit<'a> = (&'a Candidate, Vec<Range<usize>>);

#[derive(Debug, Clone, Default)]
pub struct SuggestionIndex {
    lists: HashMap<SuggestionKey, Vec<Candidate>>,
}

pub fn normalize_suggestion(value: &str) -> Option<String> {
    let value = collapse_whitespace(value);
    let chars = value.chars().count();
    (chars > 0 && chars <= MAX_VALUE_CHARS).then_some(value)
}

fn word_spans(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    text.split(' ').scan(0usize, |start, word| {
        let span = *start..*start + word.len();
        *start = span.end + 1;
        Some(span)
    })
}

fn one_span(start: usize, len: usize) -> Vec<Range<usize>> {
    std::iter::once(start..start + len).collect()
}

fn words_match(folded: &str, words: &[&str]) -> Option<Vec<Range<usize>>> {
    let mut spans = word_spans(folded).filter(|span| !span.is_empty());
    let mut ranges = Vec::with_capacity(words.len());
    for word in words {
        let span = spans.find(|span| folded[span.clone()].starts_with(word))?;
        ranges.push(span.start..span.start + word.len());
    }
    Some(ranges)
}

impl SuggestionIndex {
    pub fn build(entries: impl IntoIterator<Item = SuggestionEntry>) -> SuggestionIndex {
        let mut lists: HashMap<SuggestionKey, Vec<Candidate>> = HashMap::new();
        let mut positions: HashMap<(SuggestionKey, Box<str>), usize> = HashMap::new();
        let mut newest = 0u64;
        for entry in entries {
            let Some(value) = normalize_suggestion(&entry.value) else {
                continue;
            };
            let folded: Box<str> = fold_text(&value).into();
            if folded.trim().is_empty() {
                continue;
            }
            newest = newest.max(entry.stamp);
            let list = lists.entry(entry.key).or_default();
            match positions.get(&(entry.key, folded.clone())) {
                Some(&position) => {
                    let candidate = &mut list[position];
                    candidate.uses += 1;
                    if entry.stamp >= candidate.stamp {
                        candidate.stamp = entry.stamp;
                        candidate.value = value.into();
                    }
                }
                None => {
                    positions.insert((entry.key, folded.clone()), list.len());
                    list.push(Candidate {
                        value: value.into(),
                        folded,
                        uses: 1,
                        stamp: entry.stamp,
                        score: 0.0,
                    });
                }
            }
        }
        for list in lists.values_mut() {
            for candidate in list.iter_mut() {
                let age = newest.saturating_sub(candidate.stamp) as f32;
                let recency = 1.0 / (1.0 + age / RECENCY_HALF_LIFE);
                candidate.score = (1.0 + candidate.uses as f32).ln() + RECENCY_WEIGHT * recency;
            }
            list.sort_by(|a, b| {
                b.score
                    .total_cmp(&a.score)
                    .then_with(|| b.stamp.cmp(&a.stamp))
                    .then_with(|| a.value.cmp(&b.value))
            });
        }
        SuggestionIndex { lists }
    }

    pub fn len(&self, key: SuggestionKey) -> usize {
        self.lists.get(&key).map_or(0, Vec::len)
    }

    pub fn is_empty(&self, key: SuggestionKey) -> bool {
        self.len(key) == 0
    }

    pub fn suggest(&self, key: SuggestionKey, query: &str, limit: usize) -> Vec<Suggestion> {
        let Some(list) = self.lists.get(&key) else {
            return Vec::new();
        };
        let needle = collapse_whitespace(&fold_text(query));
        if needle.is_empty() {
            return list
                .iter()
                .take(limit)
                .map(|candidate| Suggestion {
                    value: candidate.value.to_string(),
                    uses: candidate.uses,
                    highlights: Vec::new(),
                })
                .collect();
        }
        let words: Vec<&str> = needle.split(' ').collect();
        let inside = needle.chars().count() >= 2;
        let mut found: [Vec<Hit>; 3] = Default::default();
        for candidate in list {
            if found[0].len() >= limit {
                break;
            }
            let folded = candidate.folded.as_ref();
            if folded == needle {
                continue;
            }
            let hit = if folded.starts_with(needle.as_str()) {
                Some((Tier::Prefix, one_span(0, needle.len())))
            } else if let Some(ranges) = words_match(folded, &words) {
                Some((Tier::Words, ranges))
            } else if inside {
                folded
                    .find(needle.as_str())
                    .map(|start| (Tier::Inside, one_span(start, needle.len())))
            } else {
                None
            };
            if let Some((tier, ranges)) = hit
                && found[tier as usize].len() < limit
            {
                found[tier as usize].push((candidate, ranges));
            }
        }
        found
            .into_iter()
            .flatten()
            .take(limit)
            .map(|(candidate, ranges)| {
                let folded = Folded::new(&candidate.value);
                Suggestion {
                    value: candidate.value.to_string(),
                    uses: candidate.uses,
                    highlights: ranges
                        .into_iter()
                        .map(|range| folded.source_range(range))
                        .collect(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFENCE: SuggestionKey = SuggestionKey::Fir(FirProperty::Offence);
    const ACCUSED: SuggestionKey = SuggestionKey::Fir(FirProperty::Accused);

    fn values(suggestions: &[Suggestion]) -> Vec<&str> {
        suggestions.iter().map(|s| s.value.as_str()).collect()
    }

    fn index(entries: &[(SuggestionKey, &str, u64)]) -> SuggestionIndex {
        SuggestionIndex::build(
            entries
                .iter()
                .map(|(key, value, stamp)| SuggestionEntry::new(*key, *value, *stamp)),
        )
    }

    #[test]
    fn prefix_matches_rank_before_word_and_inside_matches() {
        let index = index(&[
            (ACCUSED, "محمد علی ولد احمد", 1),
            (ACCUSED, "علی رضا", 2),
            (ACCUSED, "بلال", 3),
        ]);
        assert_eq!(
            values(&index.suggest(ACCUSED, "علی", 8)),
            ["علی رضا", "محمد علی ولد احمد"]
        );
        assert_eq!(values(&index.suggest(ACCUSED, "لا", 8)), ["بلال"]);
    }

    #[test]
    fn letter_variants_and_digits_match() {
        let index = index(&[(OFFENCE, "302/324 ت پ", 1), (ACCUSED, "علی", 1)]);
        assert_eq!(values(&index.suggest(OFFENCE, "۳۰۲", 8)), ["302/324 ت پ"]);
        let found = index.suggest(ACCUSED, "علي", 8);
        assert!(found.is_empty(), "an exact match is already typed");
        assert_eq!(values(&index.suggest(ACCUSED, "عل", 8)), ["علی"]);
    }

    #[test]
    fn every_query_word_must_start_a_word_in_order() {
        let index = index(&[(ACCUSED, "محمد علی ولد احمد خان", 1)]);
        assert_eq!(index.suggest(ACCUSED, "علی احمد", 8).len(), 1);
        assert!(index.suggest(ACCUSED, "احمد علی", 8).is_empty());
        let found = &index.suggest(ACCUSED, "عل خا", 8)[0];
        let value = &found.value;
        let marked: Vec<&str> = found
            .highlights
            .iter()
            .map(|range| &value[range.clone()])
            .collect();
        assert_eq!(marked, ["عل", "خا"]);
    }

    #[test]
    fn frequent_and_recent_values_rank_first() {
        let mut entries = vec![(OFFENCE, "379 ت پ", 1)];
        entries.extend((2..6).map(|stamp| (OFFENCE, "302 ت پ", stamp)));
        entries.push((OFFENCE, "324 ت پ", 6));
        let index = index(&entries);
        assert_eq!(
            values(&index.suggest(OFFENCE, "3", 8)),
            ["302 ت پ", "324 ت پ", "379 ت پ"]
        );
        assert_eq!(index.suggest(OFFENCE, "3", 8)[0].uses, 4);
    }

    #[test]
    fn duplicates_merge_and_keep_the_newest_spelling() {
        let index = index(&[(ACCUSED, "علي  رضا", 1), (ACCUSED, "علی رضا", 2)]);
        assert_eq!(index.len(ACCUSED), 1);
        let found = index.suggest(ACCUSED, "", 8);
        assert_eq!(values(&found), ["علی رضا"]);
        assert_eq!(found[0].uses, 2);
    }

    #[test]
    fn highlights_cover_the_original_text_when_marks_are_dropped() {
        let index = index(&[(ACCUSED, "مُحمّد علی", 1)]);
        let found = &index.suggest(ACCUSED, "محمد", 8)[0];
        assert_eq!(&found.value[found.highlights[0].clone()], "مُحمّد");
    }

    #[test]
    fn fields_are_kept_apart_and_blank_or_long_values_are_skipped() {
        let long = "ا".repeat(MAX_VALUE_CHARS + 1);
        let index = index(&[(OFFENCE, "302", 1), (ACCUSED, "  ", 1), (ACCUSED, &long, 1)]);
        assert!(index.suggest(ACCUSED, "3", 8).is_empty());
        assert!(index.is_empty(ACCUSED));
        assert_eq!(index.suggest(OFFENCE, "", 8).len(), 1);
    }

    #[test]
    fn limit_is_respected_across_tiers() {
        let entries: Vec<(SuggestionKey, String, u64)> =
            (0..40).map(|n| (ACCUSED, format!("علی {n}"), n)).collect();
        let index = SuggestionIndex::build(
            entries
                .iter()
                .map(|(key, value, stamp)| SuggestionEntry::new(*key, value.clone(), *stamp)),
        );
        let found = index.suggest(ACCUSED, "عل", SUGGESTION_LIMIT);
        assert_eq!(found.len(), SUGGESTION_LIMIT);
        assert_eq!(found[0].value, "علی 39");
    }
}
