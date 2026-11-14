ALTER TABLE `fir_records` ADD `complainant` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `writer` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `recovery` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `body` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `imei_1` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `imei_2` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `sim_1` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` ADD `sim_2` text DEFAULT '' NOT NULL;--> statement-breakpoint
ALTER TABLE `fir_records` DROP COLUMN `nic`;--> statement-breakpoint
ALTER TABLE `fir_records` DROP COLUMN `mobile`;