ALTER TABLE backlight ADD COLUMN bedtime TEXT NOT NULL DEFAULT '{"enabled":false,"start":"23:00","end":"00:00","brightness":20}';
ALTER TABLE backlight ADD COLUMN manual_override TEXT;
UPDATE backlight SET manual_override = json_object('selection', json(intended), 'expires_at', NULL);
