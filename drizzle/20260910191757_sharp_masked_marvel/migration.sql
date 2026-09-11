CREATE TABLE `app_settings` (
	`id` text PRIMARY KEY,
	`shared_placeholders` text NOT NULL,
	`updated_at` text NOT NULL,
	CONSTRAINT "app_settings_id_default" CHECK("id" = 'default')
);
--> statement-breakpoint
CREATE TABLE `fir_placeholder_values` (
	`fir_id` integer NOT NULL,
	`placeholder_id` integer NOT NULL,
	`value` text DEFAULT '' NOT NULL,
	`updated_at` text NOT NULL,
	CONSTRAINT `fir_placeholder_values_pk` PRIMARY KEY(`fir_id`, `placeholder_id`),
	CONSTRAINT `fk_fir_placeholder_values_fir_id_fir_records_id_fk` FOREIGN KEY (`fir_id`) REFERENCES `fir_records`(`id`) ON DELETE CASCADE,
	CONSTRAINT `fk_fir_placeholder_values_placeholder_id_placeholders_id_fk` FOREIGN KEY (`placeholder_id`) REFERENCES `placeholders`(`id`) ON DELETE CASCADE
);
--> statement-breakpoint
CREATE TABLE `fir_records` (
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
	`status` text NOT NULL,
	`template_id` integer,
	`content` text DEFAULT '' NOT NULL,
	CONSTRAINT `fk_fir_records_template_id_templates_id_fk` FOREIGN KEY (`template_id`) REFERENCES `templates`(`id`) ON DELETE SET NULL
);
--> statement-breakpoint
CREATE TABLE `placeholders` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`key` text NOT NULL UNIQUE,
	`label` text NOT NULL
);
--> statement-breakpoint
CREATE TABLE `templates` (
	`id` integer PRIMARY KEY AUTOINCREMENT,
	`name` text NOT NULL,
	`content` text DEFAULT '' NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `fir_placeholder_values_fir_id_idx` ON `fir_placeholder_values` (`fir_id`);--> statement-breakpoint
CREATE INDEX `fir_placeholder_values_placeholder_id_idx` ON `fir_placeholder_values` (`placeholder_id`);--> statement-breakpoint
CREATE INDEX `fir_records_status_idx` ON `fir_records` (`status`);--> statement-breakpoint
CREATE INDEX `fir_records_template_id_idx` ON `fir_records` (`template_id`);--> statement-breakpoint
CREATE INDEX `templates_updated_at_idx` ON `templates` (`updated_at`);