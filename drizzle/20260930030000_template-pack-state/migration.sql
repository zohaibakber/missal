-- Rows that only stored the Word file hash keep that hash. Null body and converter
-- columns are reconciled once by the installer, which must not restore an edit or a deletion.
ALTER TABLE `bundled_templates` ADD `body_hash` text;
--> statement-breakpoint
ALTER TABLE `bundled_templates` ADD `converter_version` integer;
--> statement-breakpoint
CREATE TABLE `template_pack_state` (
	`id` text PRIMARY KEY,
	`pack_hash` text NOT NULL,
	CONSTRAINT "template_pack_state_id_current" CHECK("id" = 'current')
);
