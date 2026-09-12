-- Keep existing placeholder IDs so saved document references continue to work.
UPDATE `placeholders`
SET `source` = '{"_tag":"SharedSetting","setting":"investigation_officer"}'
WHERE json_extract(`source`, '$.property') = 'investigation_officer';
--> statement-breakpoint
-- Preserve settings saved by older versions under camelCase keys.
UPDATE `app_settings` SET `shared_placeholders` = json_set(`shared_placeholders`, '$.police_station', json_extract(`shared_placeholders`, '$.policeStation'))
WHERE json_type(`shared_placeholders`, '$.police_station') IS NULL AND json_type(`shared_placeholders`, '$.policeStation') = 'text';
--> statement-breakpoint
UPDATE `app_settings` SET `shared_placeholders` = json_set(`shared_placeholders`, '$.sho_name', json_extract(`shared_placeholders`, '$.shoName'))
WHERE json_type(`shared_placeholders`, '$.sho_name') IS NULL AND json_type(`shared_placeholders`, '$.shoName') = 'text';
--> statement-breakpoint
UPDATE `app_settings` SET `shared_placeholders` = json_set(`shared_placeholders`, '$.dsp_name', json_extract(`shared_placeholders`, '$.dspName'))
WHERE json_type(`shared_placeholders`, '$.dsp_name') IS NULL AND json_type(`shared_placeholders`, '$.dspName') = 'text';
--> statement-breakpoint
-- Only infer an officer when every populated legacy value agrees. Keep the legacy column intact.
UPDATE `app_settings`
SET `shared_placeholders` = json_set(`shared_placeholders`, '$.investigation_officer', (SELECT min(trim(`investigation_officer`)) FROM `fir_records` WHERE trim(`investigation_officer`) <> ''))
WHERE json_type(`shared_placeholders`, '$.investigation_officer') IS NULL
AND (SELECT count(DISTINCT trim(`investigation_officer`)) FROM `fir_records` WHERE trim(`investigation_officer`) <> '') = 1;
