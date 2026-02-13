ALTER TABLE ateliers ADD COLUMN IF NOT EXISTS icon TEXT NOT NULL DEFAULT '';

UPDATE ateliers SET icon = 'coins' WHERE name = 'Accueil / Caisse';
UPDATE ateliers SET icon = 'snowplow' WHERE name = 'Dameurs';
UPDATE ateliers SET icon = 'mitten' WHERE name = 'Location';
UPDATE ateliers SET icon = 'wrench' WHERE name = 'Mécanos & Électriciens';
UPDATE ateliers SET icon = 'key' WHERE name = 'Mise en Route Matériel';
UPDATE ateliers SET icon = 'flag-checkered' WHERE name = 'Mise en Route Pistes';
UPDATE ateliers SET icon = 'snowflake' WHERE name = 'Nivoculture';
UPDATE ateliers SET icon = 'cable-car' WHERE name = 'Perchmen';
UPDATE ateliers SET icon = 'suitcase-medical' WHERE name = 'Pisteurs Secouristes';
