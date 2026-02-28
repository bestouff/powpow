CREATE TYPE equipment_type AS ENUM ('ski-slope', 'ski-tow');

CREATE TABLE IF NOT EXISTS equipments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    equipment_type equipment_type NOT NULL,
    in_service BOOL NOT NULL DEFAULT false
);

-- Seed: Pistes de ski (slopes)
INSERT INTO equipments (name, equipment_type, in_service) VALUES
    ('Ecole', 'ski-slope', false),
    ('Pierre Dorée', 'ski-slope', false),
    ('Bois Bossu', 'ski-slope', false),
    ('Les Gélinottes', 'ski-slope', false),
    ('La Rouge', 'ski-slope', false),
    ('Le Goulet', 'ski-slope', false),
    ('Le S', 'ski-slope', false),
    ('Pierre Aigue', 'ski-slope', false),
    ('La Combe', 'ski-slope', false);

-- Seed: Téléskis (tows)
INSERT INTO equipments (name, equipment_type, in_service) VALUES
    ('Pierre dorée', 'ski-tow', false),
    ('Ecole', 'ski-tow', false),
    ('Bois Bossu', 'ski-tow', false),
    ('Ruches', 'ski-tow', false),
    ('Sauzet', 'ski-tow', false);
