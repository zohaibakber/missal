use std::cmp::Ordering;

use gpui_kit::SharedString;
use missal_core::{FirId, FirRecord, FirStatus, FirSummary, format_date, parse_date};

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
    no_key: Box<str>,
    fir_key: Option<(u64, u64)>,
    date_keys: [i32; 3],
    haystack: Box<str>,
}

pub fn digit_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => c.to_digit(10),
        '\u{0660}'..='\u{0669}' => Some(c as u32 - 0x0660),
        '\u{06F0}'..='\u{06F9}' => Some(c as u32 - 0x06F0),
        _ => None,
    }
}

pub fn fold_for_search(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars() {
        match digit_value(c) {
            Some(value) => folded.push(char::from_digit(value, 10).unwrap_or(c)),
            None => folded.extend(c.to_lowercase()),
        }
    }
    folded
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

fn date_key(value: &str) -> i32 {
    parse_date(value).map_or(MISSING_DATE, |date| {
        (date.year() as i32) * 10_000 + i32::from(date.month()) * 100 + i32::from(date.day())
    })
}

impl FirRow {
    pub fn new(
        id: FirId,
        no: &str,
        date: &str,
        offence: &str,
        incident: &str,
        arrest: &str,
        status: FirStatus,
    ) -> FirRow {
        let date_text = format_date(date);
        let incident_text = format_date(incident);
        let arrest_text = format_date(arrest);
        let haystack = fold_for_search(&format!(
            "{no} {offence} {} {} {date_text} {incident_text} {arrest_text}",
            status.label(),
            status.as_str(),
        ));
        FirRow {
            id,
            no: no.to_owned().into(),
            date_keys: [date_key(date), date_key(incident), date_key(arrest)],
            no_key: natural_key(no).into(),
            fir_key: fir_number_key(no),
            haystack: haystack.into(),
            date: date_text.into(),
            offence: offence.to_owned().into(),
            incident: incident_text.into(),
            arrest: arrest_text.into(),
            status,
        }
    }

    pub fn from_summary(summary: &FirSummary) -> FirRow {
        FirRow::new(
            summary.id,
            &summary.fir_no,
            &summary.date,
            &summary.offence,
            &summary.incident_date,
            &summary.arrest_date,
            summary.status,
        )
    }

    pub fn from_record(record: &FirRecord) -> FirRow {
        FirRow::new(
            record.id,
            &record.fir.fir_no,
            &record.fir.date,
            &record.fir.offence,
            &record.fir.incident_date,
            &record.fir.arrest_date,
            record.fir.status,
        )
    }

    pub fn matches(&self, words: &[String]) -> bool {
        words.iter().all(|word| self.haystack.contains(word.as_str()))
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
        Column::Status => status_rank(a.status).cmp(&status_rank(b.status)),
        Column::Date => return compare_dates(a.date_keys[0], b.date_keys[0], a, b, sort),
        Column::IncidentDate => return compare_dates(a.date_keys[1], b.date_keys[1], a, b, sort),
        Column::ArrestDate => return compare_dates(a.date_keys[2], b.date_keys[2], a, b, sort),
    };
    let ordering = if sort.descending { ordering.reverse() } else { ordering };
    ordering.then_with(|| b.id.cmp(&a.id))
}

fn compare_numbers(a: &FirRow, b: &FirRow, sort: Sort) -> Ordering {
    let directed = |ordering: Ordering| if sort.descending { ordering.reverse() } else { ordering };
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
            let ordering = if sort.descending { ordering.reverse() } else { ordering };
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    words: Vec<String>,
    statuses: Vec<FirStatus>,
}

#[derive(Debug, Default)]
pub struct ListIndex {
    sorted: Vec<u32>,
    sorted_for: Option<(u64, Sort)>,
    shown: Vec<u32>,
    shown_for: Option<Applied>,
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
            self.shown_for = None;
        }
        let applied = Applied {
            words: search_words(query),
            statuses: statuses.to_vec(),
        };
        if self.shown_for.as_ref() == Some(&applied) {
            return &self.shown;
        }
        let narrowing = self.shown_for.as_ref().is_some_and(|previous| {
            previous.statuses == applied.statuses
                && previous
                    .words
                    .iter()
                    .all(|old| applied.words.iter().any(|word| word.contains(old.as_str())))
        });
        let matches = |row: &FirRow| {
            (statuses.is_empty() || statuses.contains(&row.status)) && row.matches(&applied.words)
        };
        let source = if narrowing { &self.shown } else { &self.sorted };
        let next: Vec<u32> = source
            .iter()
            .copied()
            .filter(|&index| matches(&rows[index as usize]))
            .collect();
        self.shown = next;
        self.shown_for = Some(applied);
        &self.shown
    }
}

pub fn derive(rows: &[FirRow], query: &str, statuses: &[FirStatus], sort: Sort) -> Vec<u32> {
    ListIndex::default().update(rows, 0, query, statuses, sort).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, no: &str, status: FirStatus, date: &str) -> FirRow {
        FirRow::new(FirId::new(id).unwrap(), no, date, "چوری ۳۷۹", date, "", status)
    }

    fn numbers(rows: &[FirRow], order: &[u32]) -> Vec<String> {
        order.iter().map(|&i| rows[i as usize].no.to_string()).collect()
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
        let ascending = derive(&rows, "", &[], Sort { column: Column::FirNo, descending: false });
        assert_eq!(
            numbers(&rows, &ascending),
            ["12/2019", "100/2025", "9/2026", "۱۰/2026", "11/2026", "12/2026"]
        );
        let descending = derive(&rows, "", &[], Sort::default());
        assert_eq!(
            numbers(&rows, &descending),
            ["12/2026", "11/2026", "۱۰/2026", "9/2026", "100/2025", "12/2019"]
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
        let ascending = derive(&rows, "", &[], Sort { column: Column::FirNo, descending: false });
        assert_eq!(numbers(&rows, &ascending), ["012/2026", "12/2026", "12/2026", "۱۲/2026"]);
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
            row(6, "99999999999999999999/2026", FirStatus::Open, "01-01-2026"),
            row(7, "1/2/3", FirStatus::Open, "01-01-2026"),
        ];
        let ascending = derive(&rows, "", &[], Sort { column: Column::FirNo, descending: false });
        let shown = numbers(&rows, &ascending);
        assert_eq!(&shown[..2], ["5/2026", "3/2027"]);
        assert_eq!(&shown[2..], ["", "1/2/3", "7-2026", "99999999999999999999/2026", "abc"]);
        let descending = derive(&rows, "", &[], Sort::default());
        let shown = numbers(&rows, &descending);
        assert_eq!(&shown[..2], ["3/2027", "5/2026"]);
        assert_eq!(&shown[2..], ["abc", "99999999999999999999/2026", "7-2026", "1/2/3", ""]);
    }

    #[test]
    fn narrowing_a_query_matches_a_fresh_derivation() {
        let mut rows = Vec::new();
        for id in 1..=300i64 {
            let status = [FirStatus::Open, FirStatus::Closed, FirStatus::UnderInvestigation][(id % 3) as usize];
            rows.push(row(id, &format!("{}/{}", id % 41 + 1, 2018 + id % 9), status, "01-01-2026"));
        }
        let sort = Sort { column: Column::FirNo, descending: true };
        let mut index = ListIndex::default();
        let queries = ["", "1", "12", "12/20", "12/2019", "12/2019 چوری", "12", "", "3", "3 closed", "3 clos", "ا"];
        for statuses in [&[][..], &[FirStatus::Closed][..]] {
            for query in queries {
                let fresh = derive(&rows, query, statuses, sort);
                assert_eq!(index.update(&rows, 1, query, statuses, sort), fresh.as_slice(), "{query:?}");
            }
        }
        rows.pop();
        assert_eq!(index.update(&rows, 2, "1", &[], sort), derive(&rows, "1", &[], sort).as_slice());
        let flipped = Sort { column: Column::FirNo, descending: false };
        assert_eq!(index.update(&rows, 2, "1", &[], flipped), derive(&rows, "1", &[], flipped).as_slice());
    }

    #[test]
    fn dates_sort_as_dates_with_blanks_last() {
        let rows = vec![
            row(1, "1", FirStatus::Open, "02-01-2026"),
            row(2, "2", FirStatus::Open, "15-12-2025"),
            row(3, "3", FirStatus::Open, "garbage"),
            row(4, "4", FirStatus::Open, "01-03-2026"),
        ];
        let ascending = derive(&rows, "", &[], Sort { column: Column::Date, descending: false });
        assert_eq!(numbers(&rows, &ascending), ["2", "1", "4", "3"]);
        let descending = derive(&rows, "", &[], Sort { column: Column::Date, descending: true });
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
        assert_eq!(derive(&rows, "closed 1/2026", &[], Sort::default()), vec![0]);
        assert_eq!(derive(&rows, "", &[FirStatus::Open], Sort::default()), vec![1]);
    }
}
