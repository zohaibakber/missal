INSERT INTO `placeholders` (`id`, `key`, `label`) VALUES
  (1, 'fir_no', 'ایف آئی آر نمبر'),
  (2, 'date', 'تاریخ ایف آئی آر'),
  (3, 'incident_date', 'تاریخ وقوعہ'),
  (4, 'arrest_date', 'تاریخ گرفتاری'),
  (5, 'offence', 'جرم'),
  (6, 'accused', 'نام ملزم و سکونت'),
  (7, 'witness', 'گواہان'),
  (8, 'nic', 'شناختی کارڈ'),
  (9, 'mobile', 'موبائل'),
  (10, 'investigation_officer', 'تفتیشی افسر'),
  (11, 'police_station', 'تھانہ نام'),
  (12, 'district', 'ضلع نام'),
  (13, 'sho_name', 'SHO نام'),
  (14, 'dsp_name', 'DSP نام');
--> statement-breakpoint
INSERT INTO `app_settings` (`id`, `shared_placeholders`, `updated_at`) VALUES
  ('default', '{}', '1970-01-01T00:00:00.000Z');
