CREATE TYPE piste_difficulty AS ENUM ('verte', 'bleue', 'rouge', 'noire');

ALTER TABLE equipments ADD COLUMN difficulty piste_difficulty;

-- Set difficulty for each ski slope
UPDATE equipments SET difficulty = 'verte' WHERE name = 'Ecole' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'verte' WHERE name = 'Pierre Dorée' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'bleue' WHERE name = 'Bois Bossu' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'bleue' WHERE name = 'Les Gélinottes' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'rouge' WHERE name = 'La Rouge' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'rouge' WHERE name = 'Le Goulet' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'bleue' WHERE name = 'Le S' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'rouge' WHERE name = 'Pierre Aigue' AND equipment_type = 'ski-slope';
UPDATE equipments SET difficulty = 'rouge' WHERE name = 'La Combe' AND equipment_type = 'ski-slope';
-- Téléskis have no difficulty (NULL)
