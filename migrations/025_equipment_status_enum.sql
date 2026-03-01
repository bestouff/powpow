CREATE TYPE equipment_status AS ENUM ('open', 'closed', 'partial');

ALTER TABLE equipments ADD COLUMN status equipment_status NOT NULL DEFAULT 'closed';

-- Migrate existing data: in_service true → open, false → closed
UPDATE equipments SET status = 'open' WHERE in_service = true;
UPDATE equipments SET status = 'closed' WHERE in_service = false;

ALTER TABLE equipments DROP COLUMN in_service;
