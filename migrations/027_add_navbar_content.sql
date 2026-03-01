-- Add navbar content block for logo + optional title
-- Add trail-map content block for the piste map image
INSERT INTO contents (slug, title, body) VALUES
('navbar', '', ''),
('trail-map', 'Plan des pistes', '')
ON CONFLICT (slug) DO NOTHING;
