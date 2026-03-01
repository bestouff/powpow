-- Add navbar content block for logo + optional title
-- Add trail-map content block for the piste map image
-- Add driving-indications content block for the olive access section
INSERT INTO contents (slug, title, body) VALUES
('navbar', '', ''),
('trail-map', 'Plan des pistes', ''),
('driving-indications', 'Accès à la station', '')
ON CONFLICT (slug) DO NOTHING;
