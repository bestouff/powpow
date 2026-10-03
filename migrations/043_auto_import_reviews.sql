-- NULL means a legacy record whose provider state was not stored. Existing
-- pending legacy records are kept for manual review below.
ALTER TABLE memberships ADD COLUMN source_state text;

CREATE TABLE auto_import_reviews (
    helloasso_item_id bigint UNIQUE REFERENCES memberships(helloasso_item_id) ON DELETE CASCADE,
    cash_id uuid UNIQUE REFERENCES cash(id) ON DELETE CASCADE,
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((helloasso_item_id IS NOT NULL) <> (cash_id IS NOT NULL))
);

-- Existing pending records may have been deliberately unimported. Leave them
-- with admins; automatic decisions apply to new records after activation.
INSERT INTO auto_import_reviews (helloasso_item_id, cash_id, reason)
SELECT m.helloasso_item_id, NULL, 'En attente avant activation de l''import automatique'
FROM memberships m
WHERE m.item_type = 'Membership'
  AND NOT EXISTS (SELECT 1 FROM payments p WHERE p.helloasso_item_id = m.helloasso_item_id);

INSERT INTO auto_import_reviews (helloasso_item_id, cash_id, reason)
SELECT NULL, c.id, 'En attente avant activation de l''import automatique'
FROM cash c
WHERE c.is_membership
  AND NOT EXISTS (SELECT 1 FROM payments p WHERE p.cash_id = c.id);
