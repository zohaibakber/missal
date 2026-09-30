-- Name fields the way the station's Word templates do (`«مقدمہ_نمبر»`). IDs are kept, so existing
-- templates and FIR documents show the new names. Names a user already changed are left alone.
UPDATE `placeholders` SET `label` = 'مقدمہ نمبر'
WHERE `label` = 'ایف آئی آر نمبر' AND NOT EXISTS (SELECT 1 FROM `placeholders` WHERE `label` = 'مقدمہ نمبر');
--> statement-breakpoint
UPDATE `placeholders` SET `label` = 'Date FIR'
WHERE `label` = 'تاریخ ایف آئی آر' AND NOT EXISTS (SELECT 1 FROM `placeholders` WHERE `label` = 'Date FIR');
--> statement-breakpoint
UPDATE `placeholders` SET `label` = 'تاریخ ووقت وقوعہ'
WHERE `label` = 'تاریخ وقوعہ' AND NOT EXISTS (SELECT 1 FROM `placeholders` WHERE `label` = 'تاریخ ووقت وقوعہ');
--> statement-breakpoint
UPDATE `placeholders` SET `label` = 'تفتیشی'
WHERE `label` = 'تفتیشی افسر' AND NOT EXISTS (SELECT 1 FROM `placeholders` WHERE `label` = 'تفتیشی');
--> statement-breakpoint
INSERT INTO `placeholders` (`label`, `source`) VALUES
  ('گواہان 1', '{"_tag":"FirProperty","property":"witness","index":1}'),
  ('گواہان 2', '{"_tag":"FirProperty","property":"witness","index":2}'),
  ('ضمنی 1', '{"_tag":"FirProperty","property":"zimni","index":1}'),
  ('ضمنی 2', '{"_tag":"FirProperty","property":"zimni","index":2}'),
  ('مدعی مقدمہ', '{"_tag":"Custom"}'),
  ('حلیہ ملزم', '{"_tag":"Custom"}'),
  ('مختصر حالات', '{"_tag":"Custom"}'),
  ('تحریر کنندہ', '{"_tag":"Custom"}'),
  ('تاریخ ضمنی', '{"_tag":"Custom"}'),
  ('چالانی ضمنی نمبر', '{"_tag":"Custom"}'),
  ('چالانی ضمنی تاریخ', '{"_tag":"Custom"}'),
  ('تاریخ2', '{"_tag":"Custom"}')
ON CONFLICT (`label`) DO NOTHING;
--> statement-breakpoint
UPDATE `app_settings` SET `field_markers` = '{"open":"«","close":"»"}'
WHERE `field_markers` = '{"open":"@","close":"@"}';
