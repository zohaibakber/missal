use std::cmp::Ordering;

use gpui_kit::SharedString;
use missal_core::{FirId, FirRecord, FirStatus, FirSummary, digit_value, parse_date};

use crate::prefs::{Column, Sort};

const MISSING_DATE: i32 = i32::MAX;
const NATURAL_DIGITS: usize = 12;

#[derive(Debug, Clone)]
pub struct FirRow {
    pub id: FirId,
    pub no: SharedString,
    pub date: SharedString,
    pub offence: SharedString,
    pub incident: SharedString,
    pub arrest: SharedString,
    pub status: FirStatus,
    pub accused: SharedString,
    pub accused_more: u32,
    no_key: Box<str>,
    fir_key: Option<(u64, u64)>,
    date_keys: [i32; 3],
    haystack: Box<str>,
}

pub fn fold_for_search(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    fold_into(&mut folded, text);
    folded
}

fn fold_into(folded: &mut String, text: &str) {
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied().unwrap_or(0);
        if byte < 0x80 {
            if byte.is_ascii_uppercase() {
                folded.push_str(&text[start..index]);
                folded.push(char::from(byte.to_ascii_lowercase()));
                start = index + 1;
            }
            index += 1;
        } else if (byte == 0xD9 && (0xA0..=0xA9).contains(&next))
            || (byte == 0xDB && (0xB0..=0xB9).contains(&next))
        {
            folded.push_str(&text[start..index]);
            folded.push(char::from(b'0' + (next & 0x0F)));
            index += 2;
            start = index;
        } else if (0xD8..=0xDB).contains(&byte) {
            index += 2;
        } else {
            let c = text[index..].chars().next().unwrap_or_default();
            let width = c.len_utf8().max(1);
            let mut lower = c.to_lowercase();
            if lower.len() != 1 || lower.next() != Some(c) {
                folded.push_str(&text[start..index]);
                folded.extend(c.to_lowercase());
                start = index + width;
            }
            index += width;
        }
    }
    folded.push_str(&text[start..]);
}

pub fn fir_number_key(text: &str) -> Option<(u64, u64)> {
    let (number, year) = text.split_once('/')?;
    let parse = |part: &str| {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        part.chars().try_fold(0u64, |value, c| {
            let digit = digit_value(c)?;
            value.checked_mul(10)?.checked_add(u64::from(digit))
        })
    };
    Some((parse(year)?, parse(number)?))
}

pub fn natural_key(text: &str) -> String {
    let mut key = String::with_capacity(text.len() + NATURAL_DIGITS);
    let mut run = String::new();
    let flush = |run: &mut String, key: &mut String| {
        if run.is_empty() {
            return;
        }
        let trimmed = run.trim_start_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed };
        for _ in digits.len()..NATURAL_DIGITS {
            key.push('0');
        }
        key.push_str(digits);
        run.clear();
    };
    for c in text.chars() {
        match digit_value(c) {
            Some(value) => run.push(char::from_digit(value, 10).unwrap_or('0')),
            None => {
                flush(&mut run, &mut key);
                key.extend(c.to_lowercase());
            }
        }
    }
    flush(&mut run, &mut key);
    key
}

fn date_parts(value: &str) -> (String, i32) {
    match parse_date(value) {
        Some(date) => (date.to_string(), date.sort_key()),
        None => (value.trim().to_owned(), MISSING_DATE),
    }
}

pub struct RowFields<'a> {
    pub no: &'a str,
    pub date: &'a str,
    pub offence: &'a str,
    pub incident: &'a str,
    pub arrest: &'a str,
    pub status: FirStatus,
    pub accused: &'a [String],
}

impl FirRow {
    pub fn new(id: FirId, fields: RowFields<'_>) -> FirRow {
        let RowFields {
            no,
            date,
            offence,
            incident,
            arrest,
            status,
            accused,
        } = fields;
        let (date_text, date_key) = date_parts(date);
        let (incident_text, incident_key) = date_parts(incident);
        let (arrest_text, arrest_key) = date_parts(arrest);
        let parts = [
            no,
            offence,
            status.label(),
            status.as_str(),
            &date_text,
            &incident_text,
            &arrest_text,
        ];
        let length = parts.iter().map(|part| part.len() + 1).sum::<usize>()
            + accused.iter().map(|name| name.len() + 1).sum::<usize>();
        let fir_key = fir_number_key(no);
        let mut haystack = String::with_capacity(length);
        for part in parts.into_iter().chain(accused.iter().map(String::as_str)) {
            if !haystack.is_empty() {
                haystack.push(' ');
            }
            fold_into(&mut haystack, part);
        }
        FirRow {
            id,
            no: no.to_owned().into(),
            date_keys: [date_key, incident_key, arrest_key],
            no_key: if fir_key.is_some() {
                Box::default()
            } else {
                natural_key(no).into()
            },
            fir_key,
            haystack: haystack.into(),
            date: date_text.into(),
            offence: offence.to_owned().into(),
            incident: incident_text.into(),
            arrest: arrest_text.into(),
            status,
            accused: accused.first().cloned().unwrap_or_default().into(),
            accused_more: accused.len().saturating_sub(1) as u32,
        }
    }

    pub fn from_summary(summary: &FirSummary) -> FirRow {
        FirRow::new(
            summary.id,
            RowFields {
                no: &summary.fir_no,
                date: &summary.date,
                offence: &summary.offence,
                incident: &summary.incident_date,
                arrest: &summary.arrest_date,
                status: summary.status,
                accused: &summary.accused,
            },
        )
    }

    pub fn from_record(record: &FirRecord) -> FirRow {
        FirRow::new(
            record.id,
            RowFields {
                no: &record.fir.fir_no,
                date: &record.fir.date,
                offence: &record.fir.offence,
                incident: &record.fir.incident_date,
                arrest: &record.fir.arrest_date,
                status: record.fir.status,
                accused: &record.fir.accused,
            },
        )
    }

    pub fn matches(&self, words: &[String]) -> bool {
        words
            .iter()
            .all(|word| self.haystack.contains(word.as_str()))
    }
}

pub fn search_words(query: &str) -> Vec<String> {
    fold_for_search(query)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn compare(a: &FirRow, b: &FirRow, sort: Sort) -> Ordering {
    let ordering = match sort.column {
        Column::FirNo => return compare_numbers(a, b, sort),
        Column::Offence => a.offence.cmp(&b.offence),
        Column::Accused => a.accused.cmp(&b.accused),
        Column::Status => status_rank(a.status).cmp(&status_rank(b.status)),
        Column::Date => return compare_dates(a.date_keys[0], b.date_keys[0], a, b, sort),
        Column::IncidentDate => return compare_dates(a.date_keys[1], b.date_keys[1], a, b, sort),
        Column::ArrestDate => return compare_dates(a.date_keys[2], b.date_keys[2], a, b, sort),
    };
    let ordering = if sort.descending {
        ordering.reverse()
    } else {
        ordering
    };
    ordering.then_with(|| b.id.cmp(&a.id))
}

fn compare_numbers(a: &FirRow, b: &FirRow, sort: Sort) -> Ordering {
    let directed = |ordering: Ordering| {
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    };
    let ordering = match (a.fir_key, b.fir_key) {
        (Some(x), Some(y)) => directed(x.cmp(&y).then_with(|| a.no.cmp(&b.no))),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => directed(a.no_key.cmp(&b.no_key).then_with(|| a.no.cmp(&b.no))),
    };
    ordering.then_with(|| b.id.cmp(&a.id))
}

fn compare_dates(x: i32, y: i32, a: &FirRow, b: &FirRow, sort: Sort) -> Ordering {
    match (x == MISSING_DATE, y == MISSING_DATE) {
        (true, true) => b.id.cmp(&a.id),
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            let ordering = x.cmp(&y);
            let ordering = if sort.descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering.then_with(|| b.id.cmp(&a.id))
        }
    }
}

fn status_rank(status: FirStatus) -> u8 {
    match status {
        FirStatus::Open => 0,
        FirStatus::UnderInvestigation => 1,
        FirStatus::ChallanSubmitted => 2,
        FirStatus::Closed => 3,
    }
}

#[derive(Debug, Default)]
pub struct ListIndex {
    sorted: Vec<u32>,
    sorted_for: Option<(u64, Sort)>,
    matched: Vec<u32>,
    matched_for: Option<Vec<String>>,
    counts: [usize; 4],
    shown: Vec<u32>,
    shown_for: Option<Vec<FirStatus>>,
}

impl ListIndex {
    pub fn update(
        &mut self,
        rows: &[FirRow],
        version: u64,
        query: &str,
        statuses: &[FirStatus],
        sort: Sort,
    ) -> &[u32] {
        if self.sorted_for != Some((version, sort)) {
            self.sorted.clear();
            self.sorted.extend(0..rows.len() as u32);
            self.sorted
                .sort_unstable_by(|&a, &b| compare(&rows[a as usize], &rows[b as usize], sort));
            self.sorted_for = Some((version, sort));
            self.matched_for = None;
        }
        let words = search_words(query);
        if self.matched_for.as_ref() != Some(&words) {
            let narrowing = self.matched_for.as_ref().is_some_and(|previous| {
                previous
                    .iter()
                    .all(|old| words.iter().any(|word| word.contains(old.as_str())))
            });
            let source = if narrowing {
                std::mem::take(&mut self.matched)
            } else {
                self.sorted.clone()
            };
            let mut counts = [0usize; 4];
            let next: Vec<u32> = source
                .into_iter()
                .filter(|&index| {
                    let row = &rows[index as usize];
                    let keep = row.matches(&words);
                    if keep {
                        counts[status_rank(row.status) as usize] += 1;
                    }
                    keep
                })
                .collect();
            self.matched = next;
            self.counts = counts;
            self.matched_for = Some(words);
            self.shown_for = None;
        }
        if self.shown_for.as_deref() != Some(statuses) {
            self.shown.clear();
            if statuses.is_empty() {
                self.shown.extend_from_slice(&self.matched);
            } else {
                self.shown.extend(
                    self.matched
                        .iter()
                        .copied()
                        .filter(|&index| statuses.contains(&rows[index as usize].status)),
                );
            }
            self.shown_for = Some(statuses.to_vec());
        }
        &self.shown
    }

    pub fn matched_count(&self) -> usize {
        self.matched.len()
    }

    pub fn status_count(&self, status: FirStatus) -> usize {
        self.counts[status_rank(status) as usize]
    }
}

pub fn derive(rows: &[FirRow], query: &str, statuses: &[FirStatus], sort: Sort) -> Vec<u32> {
    ListIndex::default()
        .update(rows, 0, query, statuses, sort)
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, no: &str, status: FirStatus, date: &str) -> FirRow {
        FirRow::new(
            FirId::new(id).unwrap(),
            RowFields {
                no,
                date,
                offence: "چوری ۳۷۹",
                incident: date,
                arrest: "",
                status,
                accused: &["زاہد حسین ولد حسین بخش".to_owned(), "طارق جاوید".to_owned()],
            },
        )
    }

    fn numbers(rows: &[FirRow], order: &[u32]) -> Vec<String> {
        order
            .iter()
            .map(|&i| rows[i as usize].no.to_string())
            .collect()
    }

    #[test]
    fn fir_numbers_sort_by_year_then_number() {
        let rows = vec![
            row(1, "12/2026", FirStatus::Open, "01-01-2026"),
            row(2, "9/2026", FirStatus::Open, "01-01-2026"),
            row(3, "100/2025", FirStatus::Open, "01-01-2026"),
            row(4, "۱۰/2026", FirStatus::Open, "01-01-2026"),
            row(5, "12/2019", FirStatus::Open, "01-01-2026"),
            row(6, "11/2026", FirStatus::Open, "01-01-2026"),
        ];
        let ascending = derive(
            &rows,
            "",
            &[],
            Sort {
                column: Column::FirNo,
                descending: false,
            },
        );
        assert_eq!(
            numbers(&rows, &ascending),
            [
                "12/2019",
                "100/2025",
                "9/2026",
                "۱۰/2026",
                "11/2026",
                "12/2026"
            ]
        );
        let descending = derive(&rows, "", &[], Sort::default());
        assert_eq!(
            numbers(&rows, &descending),
            [
                "12/2026",
                "11/2026",
                "۱۰/2026",
                "9/2026",
                "100/2025",
                "12/2019"
            ]
        );
    }

    #[test]
    fn equal_fir_numbers_fall_back_to_the_raw_string_then_the_newest_row() {
        let rows = vec![
            row(1, "12/2026", FirStatus::Open, "01-01-2026"),
            row(2, "012/2026", FirStatus::Open, "01-01-2026"),
            row(3, "۱۲/2026", FirStatus::Open, "01-01-2026"),
            row(4, "12/2026", FirStatus::Open, "01-01-2026"),
        ];
        let ascending = derive(
            &rows,
            "",
            &[],
            Sort {
                column: Column::FirNo,
                descending: false,
            },
        );
        assert_eq!(
            numbers(&rows, &ascending),
            ["012/2026", "12/2026", "12/2026", "۱۲/2026"]
        );
        assert_eq!(ascending[1], 3);
        assert_eq!(ascending[2], 0);
    }

    #[test]
    fn malformed_fir_numbers_sort_after_well_formed_ones_in_either_direction() {
        let rows = vec![
            row(1, "abc", FirStatus::Open, "01-01-2026"),
            row(2, "5/2026", FirStatus::Open, "01-01-2026"),
            row(3, "7-2026", FirStatus::Open, "01-01-2026"),
            row(4, "", FirStatus::Open, "01-01-2026"),
            row(5, "3/2027", FirStatus::Open, "01-01-2026"),
            row(
                6,
                "99999999999999999999/2026",
                FirStatus::Open,
                "01-01-2026",
            ),
            row(7, "1/2/3", FirStatus::Open, "01-01-2026"),
        ];
        let ascending = derive(
            &rows,
            "",
            &[],
            Sort {
                column: Column::FirNo,
                descending: false,
            },
        );
        let shown = numbers(&rows, &ascending);
        assert_eq!(&shown[..2], ["5/2026", "3/2027"]);
        assert_eq!(
            &shown[2..],
            ["", "1/2/3", "7-2026", "99999999999999999999/2026", "abc"]
        );
        let descending = derive(&rows, "", &[], Sort::default());
        let shown = numbers(&rows, &descending);
        assert_eq!(&shown[..2], ["3/2027", "5/2026"]);
        assert_eq!(
            &shown[2..],
            ["abc", "99999999999999999999/2026", "7-2026", "1/2/3", ""]
        );
    }

    #[test]
    fn narrowing_a_query_matches_a_fresh_derivation() {
        let mut rows = Vec::new();
        for id in 1..=300i64 {
            let status = [
                FirStatus::Open,
                FirStatus::Closed,
                FirStatus::UnderInvestigation,
            ][(id % 3) as usize];
            rows.push(row(
                id,
                &format!("{}/{}", id % 41 + 1, 2018 + id % 9),
                status,
                "01-01-2026",
            ));
        }
        let sort = Sort {
            column: Column::FirNo,
            descending: true,
        };
        let mut index = ListIndex::default();
        let queries = [
            "",
            "1",
            "12",
            "12/20",
            "12/2019",
            "12/2019 چوری",
            "12",
            "",
            "3",
            "3 closed",
            "3 clos",
            "ا",
        ];
        for statuses in [&[][..], &[FirStatus::Closed][..]] {
            for query in queries {
                let fresh = derive(&rows, query, statuses, sort);
                assert_eq!(
                    index.update(&rows, 1, query, statuses, sort),
                    fresh.as_slice(),
                    "{query:?}"
                );
            }
        }
        rows.pop();
        assert_eq!(
            index.update(&rows, 2, "1", &[], sort),
            derive(&rows, "1", &[], sort).as_slice()
        );
        let flipped = Sort {
            column: Column::FirNo,
            descending: false,
        };
        assert_eq!(
            index.update(&rows, 2, "1", &[], flipped),
            derive(&rows, "1", &[], flipped).as_slice()
        );
    }

    #[test]
    fn dates_sort_as_dates_with_blanks_last() {
        let rows = vec![
            row(1, "1", FirStatus::Open, "02-01-2026"),
            row(2, "2", FirStatus::Open, "15-12-2025"),
            row(3, "3", FirStatus::Open, "garbage"),
            row(4, "4", FirStatus::Open, "01-03-2026"),
        ];
        let ascending = derive(
            &rows,
            "",
            &[],
            Sort {
                column: Column::Date,
                descending: false,
            },
        );
        assert_eq!(numbers(&rows, &ascending), ["2", "1", "4", "3"]);
        let descending = derive(
            &rows,
            "",
            &[],
            Sort {
                column: Column::Date,
                descending: true,
            },
        );
        assert_eq!(numbers(&rows, &descending), ["4", "1", "2", "3"]);
    }

    #[test]
    fn search_matches_the_urdu_status_label_and_either_digit_set() {
        let rows = vec![
            row(1, "1/2026", FirStatus::Closed, "01-01-2026"),
            row(2, "2/2026", FirStatus::Open, "01-01-2026"),
        ];
        assert_eq!(derive(&rows, "بند", &[], Sort::default()), vec![0]);
        assert_eq!(derive(&rows, "379", &[], Sort::default()).len(), 2);
        assert_eq!(derive(&rows, "۲/2026", &[], Sort::default()), vec![1]);
        assert_eq!(
            derive(&rows, "closed 1/2026", &[], Sort::default()),
            vec![0]
        );
        assert_eq!(
            derive(&rows, "", &[FirStatus::Open], Sort::default()),
            vec![1]
        );
    }

    #[test]
    fn accused_names_are_searchable_and_counts_follow_the_search() {
        let mut rows = vec![
            row(1, "1/2026", FirStatus::Closed, "01-01-2026"),
            row(2, "2/2026", FirStatus::Open, "01-01-2026"),
            row(3, "3/2026", FirStatus::Open, "01-01-2026"),
        ];
        rows.push(FirRow::new(
            FirId::new(4).unwrap(),
            RowFields {
                no: "4/2026",
                date: "01-01-2026",
                offence: "ڈکیتی",
                incident: "01-01-2026",
                arrest: "",
                status: FirStatus::Open,
                accused: &["ندیم عباس".to_owned()],
            },
        ));
        assert_eq!(rows[0].accused.as_ref(), "زاہد حسین ولد حسین بخش");
        assert_eq!(rows[0].accused_more, 1);
        assert_eq!(rows[3].accused_more, 0);
        assert_eq!(derive(&rows, "ندیم", &[], Sort::default()), vec![3]);
        assert_eq!(derive(&rows, "طارق", &[], Sort::default()).len(), 3);
        let mut index = ListIndex::default();
        let shown = index
            .update(&rows, 1, "طارق", &[FirStatus::Open], Sort::default())
            .len();
        assert_eq!(shown, 2);
        assert_eq!(index.matched_count(), 3);
        assert_eq!(index.status_count(FirStatus::Open), 2);
        assert_eq!(index.status_count(FirStatus::Closed), 1);
        assert_eq!(index.status_count(FirStatus::ChallanSubmitted), 0);
    }
}
