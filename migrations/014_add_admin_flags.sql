ALTER TABLE staff ADD COLUMN is_admin BOOL NOT NULL DEFAULT false;
ALTER TABLE staff ADD COLUMN is_god BOOL NOT NULL DEFAULT false;
ALTER TABLE staff ADD CONSTRAINT god_implies_admin CHECK (NOT is_god OR is_admin);

UPDATE staff SET is_admin = true
  WHERE (first_name ILIKE 'Damien' AND last_name ILIKE 'Gouffault')
     OR (first_name ILIKE 'Rémy' AND last_name ILIKE 'Puech')
     OR (first_name ILIKE 'Jérôme' AND last_name ILIKE 'Folliet')
     OR (first_name ILIKE 'Guillaume' AND last_name ILIKE 'Broust');

UPDATE staff SET is_admin = true, is_god = true
  WHERE first_name ILIKE 'Xavier' AND last_name ILIKE 'Bestel';
