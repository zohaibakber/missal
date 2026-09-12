ALTER TABLE `fir_records` RENAME COLUMN `accused` TO `accused_legacy`;
--> statement-breakpoint
ALTER TABLE `fir_records` ADD COLUMN `accused` text NOT NULL DEFAULT '[]';
--> statement-breakpoint
UPDATE `fir_records` SET `accused` = json_array(`accused_legacy`);
--> statement-breakpoint
ALTER TABLE `fir_records` DROP COLUMN `accused_legacy`;
--> statement-breakpoint
ALTER TABLE `fir_records` RENAME COLUMN `witness` TO `witness_legacy`;
--> statement-breakpoint
ALTER TABLE `fir_records` ADD COLUMN `witness` text NOT NULL DEFAULT '[]';
--> statement-breakpoint
UPDATE `fir_records` SET `witness` = CASE WHEN trim(`witness_legacy`) = '' THEN '[]' ELSE json_array(`witness_legacy`) END;
--> statement-breakpoint
ALTER TABLE `fir_records` DROP COLUMN `witness_legacy`;
--> statement-breakpoint
ALTER TABLE `fir_records` ADD COLUMN `zimni` text NOT NULL DEFAULT '[]';
--> statement-breakpoint
INSERT INTO `placeholders` (`key`, `label`, `source`)
VALUES ('zimni', 'ضمنی', '{"_tag":"FirProperty","property":"zimni"}')
ON CONFLICT (`key`) DO NOTHING;
