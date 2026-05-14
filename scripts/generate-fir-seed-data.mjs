import fs from "node:fs/promises";
import path from "node:path";
import xlsx from "xlsx";

const projectRoot = process.cwd();
const workbookPath = path.join(projectRoot, "fir-data.xlsx");
const outputPath = path.join(projectRoot, "src/lib/fir-seed-data.ts");

const FIELD_MAP = {
  fir_no: "مقدمہ نمبر",
  date: "Date FIR",
  offence: "جرم ",
  accused: "نام ملزم و سکونت ",
  witness1: "گواہان  1",
  witness2: "گواہان2",
  incident_date: "تاریخ ووقت وقوعہ",
  mobile1: "سم نمبر 1",
  mobile2: "سم نمبر 2",
  complaintText: "مختصر حالات",
};

function clean(value) {
  return String(value ?? "")
    .replace(/\s+/g, " ")
    .trim();
}

function sanitizePhone(value) {
  return clean(value)
    .replace(/[^\d+\-\s]/g, "")
    .trim();
}

function extractNic(text) {
  const match = clean(text).match(/\b\d{13}\b/);
  return match?.[0] ?? "";
}

function pickFirst(...values) {
  return values.map(clean).find(Boolean) ?? "";
}

function mapRow(row, index) {
  const complaintText = clean(row[FIELD_MAP.complaintText]);
  const witness = [clean(row[FIELD_MAP.witness1]), clean(row[FIELD_MAP.witness2])]
    .filter(Boolean)
    .join(" / ");

  return {
    id: index + 1,
    fir_no: clean(row[FIELD_MAP.fir_no]),
    date: clean(row[FIELD_MAP.date]),
    offence: clean(row[FIELD_MAP.offence]),
    accused: clean(row[FIELD_MAP.accused]),
    witness,
    NIC: extractNic(complaintText),
    mobile: pickFirst(sanitizePhone(row[FIELD_MAP.mobile1]), sanitizePhone(row[FIELD_MAP.mobile2])),
    incident_date: clean(row[FIELD_MAP.incident_date]),
    arrest_date: "",
    investigation_officer: "",
    status: "Open",
  };
}

function validateRecord(record) {
  return (
    Object.values(record).every((value) => value !== undefined && value !== null) && record.fir_no
  );
}

const workbook = xlsx.readFile(workbookPath);
const sheet = workbook.Sheets[workbook.SheetNames[0]];
const rows = xlsx.utils.sheet_to_json(sheet, { defval: "" });
const records = rows.map(mapRow).filter(validateRecord);

const fileContents = `import type { FirRecord } from "#/lib/fir";\n\nexport const firSeedData: FirRecord[] = ${JSON.stringify(records, null, 2)};\n`;

await fs.writeFile(outputPath, fileContents, "utf8");
console.log(`Wrote ${records.length} FIR records to ${path.relative(projectRoot, outputPath)}`);
