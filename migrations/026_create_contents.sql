-- Content images table (CMS image storage, separate from volunteer photos)
CREATE TABLE IF NOT EXISTS content_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    data BYTEA NOT NULL,
    content_type TEXT NOT NULL,
    filename TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Editable content blocks for the frontpage CMS
CREATE TABLE IF NOT EXISTS contents (
    slug TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    image_id UUID REFERENCES content_images(id) ON DELETE SET NULL,
    link_url TEXT,
    link_label TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed the 10 content blocks with current hardcoded text as markdown
INSERT INTO contents (slug, title, body, link_url, link_label) VALUES
(
    'hero-subtitle',
    '',
    E'L''association AGH''IL \u2013 Agir pour la station de ski de St Hilaire \u2013 regroupe les bénévoles qui se mobilisent chaque hiver pour faire vivre notre belle petite station de ski.',
    NULL, NULL
),
(
    'infos-station',
    '',
    '',
    NULL, NULL
),
(
    'about-station',
    'Le Plateau des Petites Roches et sa station de ski alpin',
    E'La station de ski de Saint-Hilaire-du-Touvet est située sur le Plateau des Petites Roches, dans le massif de la Chartreuse (Isère). Nichée entre 1000 et 1300 mètres d''altitude, elle offre un panorama exceptionnel sur la vallée du Grésivaudan et la chaîne de Belledonne.\n\nDepuis sa création, la station est animée par des bénévoles passionnés qui assurent son fonctionnement : damage des pistes, exploitation des remontées mécaniques, accueil du public, vente de forfaits, secours sur pistes... Un esprit unique de solidarité et de convivialité !',
    NULL, NULL
),
(
    'about-association',
    'La station ouvre en 2026 !',
    E'QUI SOMMES-NOUS ?\n\nL''association AGH''IL (Agir pour la station de ski de St Hilaire) regroupe les bénévoles qui permettent à la station de fonctionner. Rejoignez-nous !',
    'https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil',
    'Adhérer sur HelloAsso'
),
(
    'events',
    'Événements 2026',
    E'**FÊTE de la STATION = le 07 FÉVRIER 2026**\n\nComme chaque année, la fête de la station est un moment convivial et festif ouvert à tous. Au programme : ski, animations, buvette et bonne humeur sur les pistes !',
    NULL, NULL
),
(
    'salle-hors-sac',
    'Une salle, deux utilisations...',
    E'La station met à disposition une salle hors-sac pour les skieurs et les familles. Idéale pour pique-niquer au chaud entre deux descentes, elle permet aussi d''organiser des événements associatifs.',
    NULL, NULL
),
(
    'newsletter',
    'Newsletter et adhésion',
    E'Pour rester informé de l''actualité de la station et de l''association, adhérez via HelloAsso. Vous recevrez toutes les informations sur les ouvertures et événements.',
    'https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil',
    'Adhérer sur HelloAsso'
),
(
    'footer-contact',
    'Nous contacter !',
    E'Remontées mécaniques — Tél. 04 76 08 32 20\nOffice de Tourisme — Tél. 04 76 08 33 99\nNous contacter par [e-mail](mailto:aghil.sthilaire@gmail.com)',
    NULL, NULL
),
(
    'footer-calendar',
    'Calendrier prévisionnel 2025/2026',
    E'Calendrier prévisionnel d''ouverture de la station de ski de St-Hilaire du Touvet. Sous couvert des conditions d''enneigement, et des disponibilités des bénévoles et des secouristes.\n\n- Tous les week-ends **de janvier 2026** (selon enneigement)\n- **Fête de la station** : le 07 et 08 février 2026\n- **Vacances de Février** : 2ème semaine zone A (14/02 au 22/02)\n- Fermeture au plus tard **mi-mars** 2026\n\nPour les ouvertures exceptionnelles, consultez les infos sur [notre page Facebook](https://www.facebook.com/stationskisainthilaire).',
    NULL, NULL
),
(
    'footer-summer',
    'En été',
    '',
    'https://www.funiculaire.fr',
    'Funiculaire'
)
ON CONFLICT (slug) DO NOTHING;
