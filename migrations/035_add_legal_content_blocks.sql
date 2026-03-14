-- Seed "privacy" and "tos" content blocks for CMS-editable legal pages.
INSERT INTO contents (slug, title, body) VALUES
('privacy', 'Politique de Confidentialité', ''),
('tos', 'Conditions d''Utilisation', '')
ON CONFLICT (slug) DO NOTHING;
