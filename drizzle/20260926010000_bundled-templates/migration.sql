-- Templates shipped with the app, keyed by name. A row outlives its template so one the user
-- deleted is not installed again; `template_revision` tells whether the user has edited it since.
CREATE TABLE `bundled_templates` (
	`name` text PRIMARY KEY,
	`source_hash` text NOT NULL,
	`template_id` integer,
	`template_revision` integer NOT NULL,
	CONSTRAINT `fk_bundled_templates_template_id_templates_id_fk` FOREIGN KEY (`template_id`) REFERENCES `templates`(`id`) ON DELETE SET NULL
);
