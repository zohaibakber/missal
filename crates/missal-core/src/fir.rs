use crate::error::Invalid;
use crate::ids::FirId;
use crate::text::js_trim;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FirStatus {
    #[default]
    Open,
    UnderInvestigation,
    ChallanSubmitted,
    Closed,
}

impl FirStatus {
    pub const ALL: [FirStatus; 4] = [
        FirStatus::Open,
        FirStatus::UnderInvestigation,
        FirStatus::ChallanSubmitted,
        FirStatus::Closed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            FirStatus::Open => "Open",
            FirStatus::UnderInvestigation => "Under Investigation",
            FirStatus::ChallanSubmitted => "Challan Submitted",
            FirStatus::Closed => "Closed",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            FirStatus::Open => "زیر التوا",
            FirStatus::UnderInvestigation => "زیر تفتیش",
            FirStatus::ChallanSubmitted => "چالان جمع",
            FirStatus::Closed => "بند",
        }
    }

    pub fn parse(value: &str) -> Option<FirStatus> {
        FirStatus::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FirProperty {
    FirNo,
    Complainant,
    Date,
    IncidentDate,
    ArrestDate,
    Offence,
    Accused,
    Witness,
    Zimni,
    InvestigationOfficer,
    Writer,
    Recovery,
    Body,
    Imei1,
    Imei2,
    Sim1,
    Sim2,
}

impl FirProperty {
    pub const ALL: [FirProperty; 17] = [
        FirProperty::FirNo,
        FirProperty::Complainant,
        FirProperty::Date,
        FirProperty::IncidentDate,
        FirProperty::ArrestDate,
        FirProperty::Offence,
        FirProperty::Accused,
        FirProperty::Witness,
        FirProperty::Zimni,
        FirProperty::InvestigationOfficer,
        FirProperty::Writer,
        FirProperty::Recovery,
        FirProperty::Body,
        FirProperty::Imei1,
        FirProperty::Imei2,
        FirProperty::Sim1,
        FirProperty::Sim2,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            FirProperty::FirNo => "fir_no",
            FirProperty::Complainant => "complainant",
            FirProperty::Date => "date",
            FirProperty::IncidentDate => "incident_date",
            FirProperty::ArrestDate => "arrest_date",
            FirProperty::Offence => "offence",
            FirProperty::Accused => "accused",
            FirProperty::Witness => "witness",
            FirProperty::Zimni => "zimni",
            FirProperty::InvestigationOfficer => "investigation_officer",
            FirProperty::Writer => "writer",
            FirProperty::Recovery => "recovery",
            FirProperty::Body => "body",
            FirProperty::Imei1 => "imei_1",
            FirProperty::Imei2 => "imei_2",
            FirProperty::Sim1 => "sim_1",
            FirProperty::Sim2 => "sim_2",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            FirProperty::FirNo => "ایف آئی آر نمبر",
            FirProperty::Complainant => "مدعی مقدمہ",
            FirProperty::Date => "تاریخ ایف آئی آر",
            FirProperty::IncidentDate => "تاریخ وقوعہ",
            FirProperty::ArrestDate => "تاریخ گرفتاری",
            FirProperty::Offence => "جرم",
            FirProperty::Accused => "ملزمان",
            FirProperty::Witness => "گواہان",
            FirProperty::Zimni => "ضمنی",
            FirProperty::InvestigationOfficer => "تفتیشی افسر",
            FirProperty::Writer => "تحریر کنندہ",
            FirProperty::Recovery => "برآمدگی",
            FirProperty::Body => "متن",
            FirProperty::Imei1 => "ایمی نمبر 1",
            FirProperty::Imei2 => "ایمی نمبر 2",
            FirProperty::Sim1 => "سم نمبر 1",
            FirProperty::Sim2 => "سم نمبر 2",
        }
    }

    pub fn parse(value: &str) -> Option<FirProperty> {
        FirProperty::ALL
            .into_iter()
            .find(|property| property.as_str() == value)
    }

    pub const fn is_list(self) -> bool {
        matches!(
            self,
            FirProperty::Accused | FirProperty::Witness | FirProperty::Zimni
        )
    }
}

pub const STATUS_LABEL: &str = "حالت";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirValue<'a> {
    Text(&'a str),
    List(&'a [String]),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fir {
    pub fir_no: String,
    pub complainant: String,
    pub date: String,
    pub offence: String,
    pub accused: Vec<String>,
    pub witness: Vec<String>,
    pub zimni: Vec<String>,
    pub incident_date: String,
    pub arrest_date: String,
    pub investigation_officer: String,
    pub writer: String,
    pub recovery: String,
    pub body: String,
    pub imei_1: String,
    pub imei_2: String,
    pub sim_1: String,
    pub sim_2: String,
    pub status: FirStatus,
}

impl Fir {
    pub fn value(&self, property: FirProperty) -> FirValue<'_> {
        match property {
            FirProperty::FirNo => FirValue::Text(&self.fir_no),
            FirProperty::Complainant => FirValue::Text(&self.complainant),
            FirProperty::Date => FirValue::Text(&self.date),
            FirProperty::IncidentDate => FirValue::Text(&self.incident_date),
            FirProperty::ArrestDate => FirValue::Text(&self.arrest_date),
            FirProperty::Offence => FirValue::Text(&self.offence),
            FirProperty::Accused => FirValue::List(&self.accused),
            FirProperty::Witness => FirValue::List(&self.witness),
            FirProperty::Zimni => FirValue::List(&self.zimni),
            FirProperty::InvestigationOfficer => FirValue::Text(&self.investigation_officer),
            FirProperty::Writer => FirValue::Text(&self.writer),
            FirProperty::Recovery => FirValue::Text(&self.recovery),
            FirProperty::Body => FirValue::Text(&self.body),
            FirProperty::Imei1 => FirValue::Text(&self.imei_1),
            FirProperty::Imei2 => FirValue::Text(&self.imei_2),
            FirProperty::Sim1 => FirValue::Text(&self.sim_1),
            FirProperty::Sim2 => FirValue::Text(&self.sim_2),
        }
    }

    pub fn validated(&self) -> Result<Fir, Invalid> {
        let required = |value: &str, field: &'static str| {
            let trimmed = js_trim(value);
            if trimmed.is_empty() {
                Err(Invalid::Required(field))
            } else {
                Ok(trimmed.to_owned())
            }
        };
        let optional = |value: &str| js_trim(value).to_owned();
        let entries = |values: &[String], field: &'static str| {
            values
                .iter()
                .map(|value| required(value, field).map_err(|_| Invalid::EmptyEntry(field)))
                .collect::<Result<Vec<_>, _>>()
        };
        let accused = entries(&self.accused, "accused")?;
        if accused.is_empty() {
            return Err(Invalid::NoAccused);
        }
        Ok(Fir {
            fir_no: required(&self.fir_no, "fir_no")?,
            complainant: optional(&self.complainant),
            date: required(&self.date, "date")?,
            offence: required(&self.offence, "offence")?,
            accused,
            witness: entries(&self.witness, "witness")?,
            zimni: entries(&self.zimni, "zimni")?,
            incident_date: required(&self.incident_date, "incident_date")?,
            arrest_date: optional(&self.arrest_date),
            investigation_officer: optional(&self.investigation_officer),
            writer: optional(&self.writer),
            recovery: optional(&self.recovery),
            body: optional(&self.body),
            imei_1: optional(&self.imei_1),
            imei_2: optional(&self.imei_2),
            sim_1: optional(&self.sim_1),
            sim_2: optional(&self.sim_2),
            status: self.status,
        })
    }

    pub fn empty_draft() -> Fir {
        Fir {
            accused: vec![String::new()],
            ..Fir::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirRecord {
    pub id: FirId,
    pub fir: Fir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirSummary {
    pub id: FirId,
    pub fir_no: String,
    pub date: String,
    pub offence: String,
    pub incident_date: String,
    pub arrest_date: String,
    pub status: FirStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Fir {
        Fir {
            fir_no: " 12/26 ".into(),
            date: "12-09-2026".into(),
            offence: "چوری".into(),
            incident_date: "12-09-2026".into(),
            accused: vec![" علی ".into()],
            writer: "  ".into(),
            ..Fir::default()
        }
    }

    #[test]
    fn validation_trims_and_requires() {
        let fir = draft().validated().unwrap();
        assert_eq!(fir.fir_no, "12/26");
        assert_eq!(fir.accused, ["علی"]);
        assert_eq!(fir.writer, "");
        let mut missing = draft();
        missing.offence = " ".into();
        assert_eq!(missing.validated(), Err(Invalid::Required("offence")));
        let mut empty_entry = draft();
        empty_entry.witness = vec!["".into()];
        assert_eq!(empty_entry.validated(), Err(Invalid::EmptyEntry("witness")));
        let mut no_accused = draft();
        no_accused.accused.clear();
        assert_eq!(no_accused.validated(), Err(Invalid::NoAccused));
    }

    #[test]
    fn statuses_round_trip() {
        for status in FirStatus::ALL {
            assert_eq!(FirStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(FirStatus::Closed.label(), "بند");
    }
}
