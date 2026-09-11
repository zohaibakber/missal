import { sql } from "drizzle-orm";
import { check, index, integer, primaryKey, sqliteTable, text } from "drizzle-orm/sqlite-core";

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
});

export const templates = sqliteTable(
  "templates",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    name: text("name").notNull(),
    content: text("content").notNull().default(""),
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
    accused: text("accused").notNull(),
    witness: text("witness").notNull().default(""),
    NIC: text("nic").notNull().default(""),
    mobile: text("mobile").notNull().default(""),
    incident_date: text("incident_date").notNull(),
    arrest_date: text("arrest_date").notNull().default(""),
    investigation_officer: text("investigation_officer").notNull().default(""),
    status: text("status", { enum: FIR_STATUS_VALUES }).notNull(),
    templateId: integer("template_id").references(() => templates.id, { onDelete: "set null" }),
    content: text("content").notNull().default(""),
  },
  (table) => [
    index("fir_records_status_idx").on(table.status),
    index("fir_records_template_id_idx").on(table.templateId),
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
    index("fir_placeholder_values_fir_id_idx").on(table.firId),
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
