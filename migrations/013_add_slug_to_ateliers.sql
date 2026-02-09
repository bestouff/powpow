ALTER TABLE ateliers ADD COLUMN IF NOT EXISTS slug TEXT NOT NULL DEFAULT '';

UPDATE ateliers SET slug = 'caisse' WHERE name = 'Accueil / Caisse';
UPDATE ateliers SET slug = 'dameurs' WHERE name = 'Dameurs';
UPDATE ateliers SET slug = 'location' WHERE name = 'Location';
UPDATE ateliers SET slug = 'meca' WHERE name = 'Mécanos & Électriciens';
UPDATE ateliers SET slug = 'matos' WHERE name = 'Mise en Route Matériel';
UPDATE ateliers SET slug = 'pistes' WHERE name = 'Mise en Route Pistes';
UPDATE ateliers SET slug = 'nivo' WHERE name = 'Nivoculture';
UPDATE ateliers SET slug = 'perchmen' WHERE name = 'Perchmen';
UPDATE ateliers SET slug = 'pisteurs' WHERE name = 'Pisteurs Secouristes';

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ateliers_slug_unique') THEN
        ALTER TABLE ateliers ADD CONSTRAINT ateliers_slug_unique UNIQUE (slug);
    END IF;
END $$;
