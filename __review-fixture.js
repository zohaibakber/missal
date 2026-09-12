import { emptyDocumentEnvelope } from "/src/lib/document-format.ts";
import { createDefaultPlaceholders } from "/src/lib/placeholder.ts";
const timestamp = "2026-09-12T00:00:00.000Z";
const names = [
  "درخواست ریمانڈ",
  "بیان گواہ",
  "رپورٹ تفتیش",
  "ضمنی رپورٹ",
  "فرد گرفتاری",
  "درخواست ضمانت",
];
const envelope = () => {
  const doc = structuredClone(emptyDocumentEnvelope());
  doc.state.root.children[0].children = [
    {
      type: "text",
      version: 1,
      text: "جناب عالی، مقدمہ کی تفتیش کے دوران درج ذیل حقائق سامنے آئے۔",
      format: 0,
      mode: "normal",
      style: "",
      detail: 0,
    },
  ];
  return doc;
};
let templates = Array.from({ length: 26 }, (_, i) => ({
  id: i + 1,
  name: names[i % names.length] + (i > 5 ? ` ${i + 1}` : ""),
  document: envelope(),
  revision: 1,
  createdAt: timestamp,
  updatedAt: timestamp,
  previewText: "جناب عالی، مقدمہ کی تفتیش کے دوران درج ذیل حقائق سامنے آئے۔",
  fieldCount: 0,
}));
let placeholders = createDefaultPlaceholders().map((item, i) => ({
  ...item,
  id: i + 1,
  source: { _tag: "Custom" },
}));
let firs = [
  {
    id: 1,
    fir_no: "TEST/26",
    date: "12-09-2026",
    incident_date: "11-09-2026",
    arrest_date: "",
    offence: "آزمائشی مقدمہ",
    accused: ["آزمائشی ملزم اول", "آزمائشی ملزم دوم"],
    witness: ["آزمائشی گواہ"],
    zimni: ["آزمائشی ضمنی"],
    NIC: "",
    mobile: "",
    investigation_officer: "",
    status: "Open",
  },
];
window.electronStorage = {
  request: async (request) => {
    let value;
    switch (request._tag) {
      case "Placeholder.list":
        value = placeholders;
        break;
      case "Placeholder.create":
        value = {
          ...request.input,
          id: placeholders.length + 1,
          key: `custom_${placeholders.length + 1}`,
          source: { _tag: "Custom" },
        };
        placeholders.push(value);
        break;
      case "Placeholder.update":
        value = { ...placeholders.find((p) => p.id === request.input.id), ...request.input };
        placeholders = placeholders.map((p) => (p.id === value.id ? value : p));
        break;
      case "Template.list":
        value = templates;
        break;
      case "Template.get":
        value = templates.find((p) => p.id === request.id);
        break;
      case "Template.create":
        value = {
          ...request.input,
          id: templates.length + 1,
          revision: 1,
          createdAt: timestamp,
          updatedAt: timestamp,
          previewText: "",
          fieldCount: 0,
        };
        templates.push(value);
        break;
      case "Template.save":
        value = {
          ...templates.find((p) => p.id === request.input.id),
          ...request.input,
          revision: request.input.expectedRevision + 1,
        };
        templates = templates.map((p) => (p.id === value.id ? value : p));
        break;
      case "Fir.list":
        value = firs;
        break;
      case "Fir.get":
        value = firs.find((f) => f.id === request.id);
        break;
      case "Fir.create":
        value = { ...request.input, id: firs.length + 1 };
        firs.push(value);
        break;
      case "Fir.update":
        value = request.input;
        firs = firs.map((f) => (f.id === value.id ? value : f));
        break;
      case "Fir.valueContext":
        value = {
          fir: firs.find((f) => f.id === request.id),
          catalog: placeholders,
          overrides: [],
          sharedSettings: {},
        };
        break;
      case "FirDocument.listForFir":
        value = [];
        break;
      case "Settings.get":
        value = { id: "default", sharedPlaceholders: {}, updatedAt: timestamp };
        break;
      default:
        return {
          _tag: "Failure",
          error: { _tag: "StorageError", message: "Fixture unavailable", operation: request._tag },
        };
    }
    return { _tag: "Success", value: structuredClone(value) };
  },
};
await import("/src/renderer.tsx");
