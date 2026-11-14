use std::path::Path;

use missal_core::{CalendarDate, Fir, FirStatus, RepositoryError, TemplateId};
use missal_store::Store;
use rusqlite::params;

const OFFENCES: [&str; 12] = [
    "چوری ۳۷۹ ت پ",
    "ڈکیتی ۳۹۲ ت پ",
    "قتل عمد ۳۰۲ ت پ",
    "اقدام قتل ۳۲۴ ت پ",
    "منشیات ۹ سی",
    "راہزنی ۳۹۴ ت پ",
    "فراڈ ۴۲۰ ت پ",
    "اغوا ۳۶۳ ت پ",
    "غیر قانونی اسلحہ ۱۵ اے",
    "لڑائی جھگڑا ۳۳۷ ت پ",
    "نقب زنی ۴۵۷ ت پ",
    "دھوکہ دہی اور جعل سازی ۴۶۸ ت پ",
];

const NAMES: [&str; 10] = [
    "محمد اسلم ولد اکرم سکنہ چک ۴۵",
    "علی رضا ولد حسن سکنہ محلہ اسلام پورہ",
    "عمران خان ولد شیر خان سکنہ گوجرانوالہ",
    "فیصل محمود ولد محمود احمد سکنہ ملتان",
    "ندیم عباس ولد غلام عباس سکنہ لاہور",
    "طارق جاوید ولد جاوید اقبال سکنہ فیصل آباد",
    "شاہد نواز ولد نواز علی سکنہ بہاولپور",
    "کامران اکبر ولد اکبر علی سکنہ سیالکوٹ",
    "زاہد حسین ولد حسین بخش سکنہ ساہیوال",
    "اظہر اقبال ولد اقبال حسین سکنہ جھنگ",
];

const OFFICERS: [&str; 5] = [
    "سب انسپکٹر محمد یوسف",
    "انسپکٹر ملک ارشد",
    "اے ایس آئی رشید احمد",
    "سب انسپکٹر عبدالستار",
    "انسپکٹر چوہدری نذیر",
];

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn date(random: &mut Random, year: i64) -> CalendarDate {
    let month = 1 + random.below(12) as u8;
    let day = 1 + random.below(28) as u8;
    CalendarDate::new(year, month, day).expect("a valid calendar date")
}

fn sample_fir(index: usize, random: &mut Random) -> Fir {
    let year = 2019 + (index % 8) as i64;
    let number = 1 + (index / 8) % 3000;
    let incident = date(random, year);
    let status = match random.below(10) {
        0..=3 => FirStatus::Open,
        4..=6 => FirStatus::UnderInvestigation,
        7 | 8 => FirStatus::ChallanSubmitted,
        _ => FirStatus::Closed,
    };
    let accused_count = 1 + random.below(3);
    let witness_count = random.below(3);
    let name = |random: &mut Random| NAMES[random.below(NAMES.len())].to_owned();
    let arrested =
        matches!(status, FirStatus::ChallanSubmitted | FirStatus::Closed) || random.below(4) == 0;
    Fir {
        fir_no: format!("{number}/{year}"),
        complainant: name(random),
        date: incident.to_string(),
        offence: OFFENCES[random.below(OFFENCES.len())].to_owned(),
        accused: (0..accused_count).map(|_| name(random)).collect(),
        witness: (0..witness_count).map(|_| name(random)).collect(),
        zimni: if random.below(3) == 0 {
            vec!["موقع واردات کا نقشہ تیار کیا گیا".to_owned()]
        } else {
            Vec::new()
        },
        incident_date: incident.to_string(),
        arrest_date: if arrested {
            date(random, year).to_string()
        } else {
            String::new()
        },
        investigation_officer: OFFICERS[random.below(OFFICERS.len())].to_owned(),
        writer: OFFICERS[random.below(OFFICERS.len())].to_owned(),
        recovery: if random.below(3) == 0 {
            "ایک عدد موٹر سائیکل برآمد کی گئی".to_owned()
        } else {
            String::new()
        },
        body: "مدعی نے تھانہ میں درخواست دی کہ نامعلوم ملزمان رات کے وقت گھر میں داخل ہو کر قیمتی سامان لے گئے۔"
            .to_owned(),
        imei_1: if random.below(5) == 0 {
            format!("35{:013}", random.next() % 10_000_000_000_000)
        } else {
            String::new()
        },
        imei_2: String::new(),
        sim_1: if random.below(5) == 0 {
            format!("03{:09}", random.next() % 1_000_000_000)
        } else {
            String::new()
        },
        sim_2: String::new(),
        status,
    }
}

pub fn seed_firs(database: &Path, count: usize) -> Result<usize, RepositoryError> {
    let mut store = Store::open(database)?;
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let first = count.min(6);
    let templates = store.templates()?;
    let template_ids: Vec<TemplateId> = templates
        .iter()
        .take(4)
        .map(|template| template.id)
        .collect();
    for index in 0..first {
        let record = store.create_fir(&sample_fir(index, &mut random))?;
        if index < 3 {
            store.add_templates_to_fir(
                record.id,
                &template_ids[..template_ids.len().min(1 + index)],
            )?;
        }
    }
    let mut connection = store.into_connection();
    let failed = |_| RepositoryError::storage("seed", "Storage operation failed");
    let transaction = connection.transaction().map_err(failed)?;
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO fir_records (fir_no, complainant, date, offence, accused, witness, zimni, incident_date, arrest_date, investigation_officer, writer, recovery, body, imei_1, imei_2, sim_1, sim_2, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            )
            .map_err(failed)?;
        for index in first..count {
            let fir = sample_fir(index, &mut random);
            let list = |items: &[String]| serde_json::to_string(items).unwrap_or_default();
            statement
                .execute(params![
                    fir.fir_no,
                    fir.complainant,
                    fir.date,
                    fir.offence,
                    list(&fir.accused),
                    list(&fir.witness),
                    list(&fir.zimni),
                    fir.incident_date,
                    fir.arrest_date,
                    fir.investigation_officer,
                    fir.writer,
                    fir.recovery,
                    fir.body,
                    fir.imei_1,
                    fir.imei_2,
                    fir.sim_1,
                    fir.sim_2,
                    fir.status.as_str(),
                ])
                .map_err(failed)?;
        }
    }
    transaction.commit().map_err(failed)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_valid_records_quickly() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missal.sqlite");
        assert_eq!(seed_firs(&path, 3000).unwrap(), 3000);
        let store = Store::open(&path).unwrap();
        let firs = store.firs().unwrap();
        assert_eq!(firs.len(), 3000);
        assert!(firs.iter().any(|fir| fir.status == FirStatus::Closed));
        let first = store.fir(firs[firs.len() - 1].id).unwrap();
        assert!(first.fir.validated().is_ok());
        assert!(
            !store
                .fir_documents(firs[firs.len() - 1].id)
                .unwrap()
                .is_empty()
        );
    }
}
