-- A placeholder is identified by its name alone. Names must be unique so templates can refer to them.
UPDATE `placeholders` SET `label` = trim(`label`);
--> statement-breakpoint
UPDATE `placeholders` SET `label` = `label` || ' ' || `id`
WHERE EXISTS (SELECT 1 FROM `placeholders` AS `earlier` WHERE `earlier`.`label` = `placeholders`.`label` AND `earlier`.`id` < `placeholders`.`id`);
--> statement-breakpoint
-- SQLite cannot drop a UNIQUE column, so rebuild the table. Dropping it cascades to FIR values; keep a copy.
CREATE TABLE `fir_placeholder_values_backup` AS SELECT * FROM `fir_placeholder_values`;
--> statement-breakpoint
CREATE TABLE `placeholders_new` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`label` text NOT NULL UNIQUE,
	`source` text NOT NULL DEFAULT '{"_tag":"Custom"}'
);
--> statement-breakpoint
INSERT INTO `placeholders_new` (`id`, `label`, `source`) SELECT `id`, `label`, `source` FROM `placeholders`;
--> statement-breakpoint
DROP TABLE `placeholders`;
--> statement-breakpoint
ALTER TABLE `placeholders_new` RENAME TO `placeholders`;
--> statement-breakpoint
INSERT OR IGNORE INTO `fir_placeholder_values` (`fir_id`, `placeholder_id`, `value`, `updated_at`)
SELECT `fir_id`, `placeholder_id`, `value`, `updated_at` FROM `fir_placeholder_values_backup`;
--> statement-breakpoint
DROP TABLE `fir_placeholder_values_backup`;
--> statement-breakpoint
ALTER TABLE `app_settings` ADD COLUMN `field_markers` text NOT NULL DEFAULT '{"open":"@","close":"@"}';
