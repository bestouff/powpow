-- Rename frontpage → is_frontpage and add is_staff boolean
ALTER TABLE photos RENAME COLUMN frontpage TO is_frontpage;
ALTER TABLE photos ADD COLUMN is_staff BOOLEAN NOT NULL DEFAULT FALSE;
