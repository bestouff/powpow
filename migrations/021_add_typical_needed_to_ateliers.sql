ALTER TABLE ateliers ADD COLUMN IF NOT EXISTS opening_day_typical_needed SMALLINT NOT NULL DEFAULT 0;

UPDATE ateliers SET opening_day_typical_needed = 4 WHERE slug = 'caisse';
UPDATE ateliers SET opening_day_typical_needed = 5 WHERE slug = 'location';
UPDATE ateliers SET opening_day_typical_needed = 1 WHERE slug = 'meca';
UPDATE ateliers SET opening_day_typical_needed = 5 WHERE slug = 'perchmen';
UPDATE ateliers SET opening_day_typical_needed = 2 WHERE slug = 'pisteurs';
