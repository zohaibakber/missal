PRAGMA foreign_keys = OFF;
--> statement-breakpoint
ALTER TABLE `placeholders` ADD COLUMN `source` text NOT NULL DEFAULT '{"_tag":"Custom"}';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"fir_no"}' WHERE `key` = 'fir_no';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"date"}' WHERE `key` = 'date';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"incident_date"}' WHERE `key` = 'incident_date';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"arrest_date"}' WHERE `key` = 'arrest_date';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"offence"}' WHERE `key` = 'offence';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"accused"}' WHERE `key` = 'accused';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"witness"}' WHERE `key` = 'witness';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"NIC"}' WHERE `key` = 'nic';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"mobile"}' WHERE `key` = 'mobile';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"FirProperty","property":"investigation_officer"}' WHERE `key` = 'investigation_officer';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"SharedSetting","setting":"police_station"}' WHERE `key` = 'police_station';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"SharedSetting","setting":"district"}' WHERE `key` = 'district';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"SharedSetting","setting":"sho_name"}' WHERE `key` = 'sho_name';
--> statement-breakpoint
UPDATE `placeholders` SET `source` = '{"_tag":"SharedSetting","setting":"dsp_name"}' WHERE `key` = 'dsp_name';
--> statement-breakpoint
CREATE TABLE `fir_records_new` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`fir_no` text NOT NULL,
	`date` text NOT NULL,
	`offence` text NOT NULL,
	`accused` text NOT NULL,
	`witness` text DEFAULT '' NOT NULL,
	`nic` text DEFAULT '' NOT NULL,
	`mobile` text DEFAULT '' NOT NULL,
	`incident_date` text NOT NULL,
	`arrest_date` text DEFAULT '' NOT NULL,
	`investigation_officer` text DEFAULT '' NOT NULL,
	`status` text NOT NULL
);
--> statement-breakpoint
INSERT INTO `fir_records_new` (
	`id`, `fir_no`, `date`, `offence`, `accused`, `witness`, `nic`, `mobile`, `incident_date`, `arrest_date`, `investigation_officer`, `status`
)
SELECT
	`id`, `fir_no`, `date`, `offence`, `accused`, `witness`, `nic`, `mobile`, `incident_date`, `arrest_date`, `investigation_officer`, `status`
FROM `fir_records`;
--> statement-breakpoint
DROP TABLE `fir_records`;
--> statement-breakpoint
ALTER TABLE `fir_records_new` RENAME TO `fir_records`;
--> statement-breakpoint
CREATE INDEX `fir_records_status_idx` ON `fir_records` (`status`);
--> statement-breakpoint
CREATE TABLE `templates_new` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`name` text NOT NULL,
	`document` text NOT NULL,
	`revision` integer DEFAULT 1 NOT NULL,
	`plain_text` text DEFAULT '' NOT NULL,
	`preview_text` text DEFAULT '' NOT NULL,
	`field_references` text DEFAULT '[]' NOT NULL,
	`field_count` integer DEFAULT 0 NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL
);
--> statement-breakpoint
INSERT INTO `templates_new` (`id`, `name`, `document`, `revision`, `plain_text`, `preview_text`, `field_references`, `field_count`, `created_at`, `updated_at`)
SELECT
	`id`,
	`name`,
	'{"format":"missal-lexical","version":1,"state":{"root":{"children":[{"children":[],"direction":"rtl","format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":"rtl","format":"","indent":0,"type":"root","version":1}}}',
	1,
	'',
	'',
	'[]',
	0,
	`created_at`,
	`updated_at`
FROM `templates`;
--> statement-breakpoint
DROP TABLE `templates`;
--> statement-breakpoint
ALTER TABLE `templates_new` RENAME TO `templates`;
--> statement-breakpoint
CREATE INDEX `templates_updated_at_idx` ON `templates` (`updated_at`);
--> statement-breakpoint
CREATE INDEX `templates_plain_text_idx` ON `templates` (`plain_text`);
--> statement-breakpoint
CREATE TABLE `fir_documents` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`fir_id` integer NOT NULL,
	`template_id` integer NOT NULL,
	`source_template_revision` integer NOT NULL,
	`title` text NOT NULL,
	`document` text NOT NULL,
	`revision` integer DEFAULT 1 NOT NULL,
	`position` integer DEFAULT 0 NOT NULL,
	`plain_text` text DEFAULT '' NOT NULL,
	`preview_text` text DEFAULT '' NOT NULL,
	`field_references` text DEFAULT '[]' NOT NULL,
	`field_count` integer DEFAULT 0 NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	CONSTRAINT `fk_fir_documents_fir_id_fir_records_id_fk` FOREIGN KEY (`fir_id`) REFERENCES `fir_records`(`id`) ON DELETE CASCADE,
	CONSTRAINT `fk_fir_documents_template_id_templates_id_fk` FOREIGN KEY (`template_id`) REFERENCES `templates`(`id`) ON DELETE RESTRICT
);
--> statement-breakpoint
CREATE UNIQUE INDEX `fir_documents_fir_id_template_id_uidx` ON `fir_documents` (`fir_id`, `template_id`);
--> statement-breakpoint
CREATE INDEX `fir_documents_fir_id_position_idx` ON `fir_documents` (`fir_id`, `position`);
--> statement-breakpoint
CREATE INDEX `fir_documents_template_id_idx` ON `fir_documents` (`template_id`);
--> statement-breakpoint
PRAGMA foreign_keys = ON;
