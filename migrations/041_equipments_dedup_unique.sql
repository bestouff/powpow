-- Fix doubled equipment rows: migration 023's seed INSERT is not idempotent and,
-- when equipments already contained data, a second set of rows was inserted.
-- Keep one row per (name, equipment_type) and forbid future duplicates.

-- Deduplicate: keep the row with the smallest id for each (name, equipment_type).
DELETE FROM equipments a
USING equipments b
WHERE a.name = b.name
  AND a.equipment_type = b.equipment_type
  AND a.id > b.id;

-- The natural key of an equipment is its name within its type.
ALTER TABLE equipments ADD CONSTRAINT equipments_name_type_key UNIQUE (name, equipment_type);

-- A qualification type is identified by its name.
CREATE UNIQUE INDEX IF NOT EXISTS qualifications_name_key ON qualifications (name);

-- Atelier display names must be unique.
CREATE UNIQUE INDEX IF NOT EXISTS ateliers_name_key ON ateliers (name);