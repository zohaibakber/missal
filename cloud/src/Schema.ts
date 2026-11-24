import { index, integer, primaryKey, sqliteTable, text } from "drizzle-orm/sqlite-core";

export const users = sqliteTable("users", {
  id: text("id").primaryKey(),
  email: text("email").notNull().unique(),
  name: text("name").notNull(),
  role: text("role", { enum: ["admin", "publisher", "member"] }).notNull(),
  password: text("password").notNull().default(""),
  disabled: integer("disabled").notNull().default(0),
  emailVerifiedAt: integer("email_verified_at"),
  trialEndsAt: integer("trial_ends_at"),
  paidUntil: integer("paid_until"),
  createdAt: integer("created_at").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const sessions = sqliteTable(
  "sessions",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id, { onDelete: "cascade" }),
    refreshHash: text("refresh_hash").notNull(),
    createdAt: integer("created_at").notNull(),
    lastUsedAt: integer("last_used_at").notNull(),
    expiresAt: integer("expires_at").notNull(),
  },
  (table) => [index("sessions_user").on(table.userId)],
);

export const signInFailures = sqliteTable("sign_in_failures", {
  email: text("email").primaryKey(),
  failures: integer("failures").notNull(),
  lockedUntil: integer("locked_until").notNull(),
});

export const emailCodes = sqliteTable(
  "email_codes",
  {
    email: text("email").notNull(),
    purpose: text("purpose", { enum: ["verify", "reset", "notice"] }).notNull(),
    codeHash: text("code_hash").notNull(),
    expiresAt: integer("expires_at").notNull(),
    attempts: integer("attempts").notNull().default(0),
    sentAt: integer("sent_at").notNull(),
  },
  (table) => [primaryKey({ columns: [table.email, table.purpose] })],
);

export const templates = sqliteTable("templates", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  version: integer("version").notNull(),
  fieldCount: integer("field_count").notNull(),
  preview: text("preview").notNull(),
  size: integer("size").notNull(),
  publishedBy: text("published_by").notNull(),
  createdAt: integer("created_at").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const templateVersions = sqliteTable(
  "template_versions",
  {
    templateId: text("template_id").notNull(),
    version: integer("version").notNull(),
    name: text("name").notNull(),
    fieldCount: integer("field_count").notNull(),
    preview: text("preview").notNull(),
    size: integer("size").notNull(),
    objectKey: text("object_key").notNull(),
    publishedBy: text("published_by").notNull(),
    publishedAt: integer("published_at").notNull(),
    restoredFrom: integer("restored_from"),
  },
  (table) => [primaryKey({ columns: [table.templateId, table.version] })],
);
