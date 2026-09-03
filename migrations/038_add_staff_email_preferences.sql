ALTER TABLE staff
    ADD COLUMN no_import_emails BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN no_weekly_emails BOOLEAN NOT NULL DEFAULT false;
