import { sql } from "drizzle-orm";
import {
  check,
  index,
  integer,
  primaryKey,
  sqliteTable,
  text,
  uniqueIndex,
} from "drizzle-orm/sqlite-core";
import type { DocumentEnvelope } from "#/lib/document-format";
import type { FieldReference, FieldSource } from "#/lib/field";

export const FIR_STATUS_VALUES = [
  "Open",
  "Under Investigation",
  "Challan Submitted",
  "Closed",
] as const;

export const placeholders = sqliteTable("placeholders", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  key: text("key").notNull().unique(),
  label: text("label").notNull(),
  source: text("source", { mode: "json" }).notNull().$type<FieldSource>(),
});

export const templates = sqliteTable(
  "templates",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    name: text("name").notNull(),
    document: text("document", { mode: "json" }).notNull().$type<DocumentEnvelope>(),
    revision: integer("revision").notNull().default(1),
    plainText: text("plain_text").notNull().default(""),
    previewText: text("preview_text").notNull().default(""),
    fieldReferences: text("field_references", { mode: "json" })
      .notNull()
      .$type<readonly FieldReference[]>()
      .default([]),
    fieldCount: integer("field_count").notNull().default(0),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [index("templates_updated_at_idx").on(table.updatedAt)],
);

export const firRecords = sqliteTable(
  "fir_records",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    fir_no: text("fir_no").notNull(),
    date: text("date").notNull(),
    offence: text("offence").notNull(),
    accused: text("accused", { mode: "json" }).notNull().$type<readonly string[]>().default([]),
    witness: text("witness", { mode: "json" }).notNull().$type<readonly string[]>().default([]),
    zimni: text("zimni", { mode: "json" }).notNull().$type<readonly string[]>().default([]),
    NIC: text("nic").notNull().default(""),
    mobile: text("mobile").notNull().default(""),
    incident_date: text("incident_date").notNull(),
    arrest_date: text("arrest_date").notNull().default(""),
    investigation_officer: text("investigation_officer").notNull().default(""),
    status: text("status", { enum: FIR_STATUS_VALUES }).notNull(),
  },
  (table) => [index("fir_records_status_idx").on(table.status)],
);

export const firDocuments = sqliteTable(
  "fir_documents",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    firId: integer("fir_id")
      .notNull()
      .references(() => firRecords.id, { onDelete: "cascade" }),
    templateId: integer("template_id")
      .notNull()
      .references(() => templates.id, { onDelete: "restrict" }),
    sourceTemplateRevision: integer("source_template_revision").notNull(),
    title: text("title").notNull(),
    document: text("document", { mode: "json" }).notNull().$type<DocumentEnvelope>(),
    revision: integer("revision").notNull().default(1),
    position: integer("position").notNull().default(0),
    plainText: text("plain_text").notNull().default(""),
    previewText: text("preview_text").notNull().default(""),
    fieldReferences: text("field_references", { mode: "json" })
      .notNull()
      .$type<readonly FieldReference[]>()
      .default([]),
    fieldCount: integer("field_count").notNull().default(0),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    uniqueIndex("fir_documents_fir_id_template_id_uidx").on(table.firId, table.templateId),
    index("fir_documents_fir_id_position_idx").on(table.firId, table.position),
    index("fir_documents_template_id_idx").on(table.templateId),
  ],
);

export const firPlaceholderValues = sqliteTable(
  "fir_placeholder_values",
  {
    firId: integer("fir_id")
      .notNull()
      .references(() => firRecords.id, { onDelete: "cascade" }),
    placeholderId: integer("placeholder_id")
      .notNull()
      .references(() => placeholders.id, { onDelete: "cascade" }),
    value: text("value").notNull().default(""),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    primaryKey({ columns: [table.firId, table.placeholderId] }),
    index("fir_placeholder_values_placeholder_id_idx").on(table.placeholderId),
  ],
);

export const appSettings = sqliteTable(
  "app_settings",
  {
    id: text("id").primaryKey(),
    sharedPlaceholders: text("shared_placeholders", { mode: "json" })
      .notNull()
      .$type<Record<string, string>>(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [check("app_settings_id_default", sql`${table.id} = 'default'`)],
);

export const templateSummaryColumns = {
  id: templates.id,
  name: templates.name,
  revision: templates.revision,
  previewText: templates.previewText,
  fieldCount: templates.fieldCount,
  createdAt: templates.createdAt,
  updatedAt: templates.updatedAt,
};

export const firDocumentSummaryColumns = {
  id: firDocuments.id,
  firId: firDocuments.firId,
  templateId: firDocuments.templateId,
  sourceTemplateRevision: firDocuments.sourceTemplateRevision,
  title: firDocuments.title,
  revision: firDocuments.revision,
  position: firDocuments.position,
  previewText: firDocuments.previewText,
  fieldCount: firDocuments.fieldCount,
  createdAt: firDocuments.createdAt,
  updatedAt: firDocuments.updatedAt,
};
