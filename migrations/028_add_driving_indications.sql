-- Add driving-indications content block for the olive access section
INSERT INTO contents (slug, title, body) VALUES
('driving-indications', 'Accès à la station', '')
ON CONFLICT (slug) DO NOTHING;
