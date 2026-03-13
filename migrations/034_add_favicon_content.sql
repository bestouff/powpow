-- Seed the "favicon" content block (image used as site favicon).
INSERT INTO contents (slug, title, body) VALUES
('favicon', 'Favicon', '')
ON CONFLICT (slug) DO NOTHING;
