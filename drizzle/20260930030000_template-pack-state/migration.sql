ALTER TABLE `bundled_templates` RENAME COLUMN `source_hash` TO `body_hash`;
--> statement-breakpoint
-- The last pack whose every entry was applied; an unchanged pack skips synchronization.
CREATE TABLE `template_pack_state` (
	`id` text PRIMARY KEY,
	`pack_hash` text NOT NULL,
	CONSTRAINT "template_pack_state_id_current" CHECK("id" = 'current')
);
