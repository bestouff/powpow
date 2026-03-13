-- Seed the "pricing" content block for the Tarifs section on the home page.
INSERT INTO contents (slug, title, body) VALUES
('pricing', 'Tarifs', '')
ON CONFLICT (slug) DO NOTHING;
