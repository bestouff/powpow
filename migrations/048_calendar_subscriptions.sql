CREATE TABLE calendar_subscriptions (
    staff uuid PRIMARY KEY REFERENCES staff(id) ON DELETE CASCADE,
    token text NOT NULL UNIQUE CHECK (length(token) = 43)
);

-- Calendar clients need stable modification dates across repeated downloads.
ALTER TABLE presence ADD COLUMN calendar_updated_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE needs ADD COLUMN calendar_updated_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE ateliers ADD COLUMN calendar_updated_at timestamptz NOT NULL DEFAULT now();

CREATE FUNCTION touch_calendar_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.calendar_updated_at = clock_timestamp();
    RETURN NEW;
END;
$$;

CREATE TRIGGER presence_calendar_updated
BEFORE UPDATE ON presence FOR EACH ROW
WHEN (OLD.first_half IS DISTINCT FROM NEW.first_half OR OLD.second_half IS DISTINCT FROM NEW.second_half)
EXECUTE FUNCTION touch_calendar_updated_at();

CREATE TRIGGER needs_calendar_updated
BEFORE UPDATE ON needs FOR EACH ROW
WHEN (OLD.day IS DISTINCT FROM NEW.day OR OLD.atelier IS DISTINCT FROM NEW.atelier OR OLD.nightly IS DISTINCT FROM NEW.nightly)
EXECUTE FUNCTION touch_calendar_updated_at();

CREATE TRIGGER ateliers_calendar_updated
BEFORE UPDATE ON ateliers FOR EACH ROW
WHEN (OLD.name IS DISTINCT FROM NEW.name)
EXECUTE FUNCTION touch_calendar_updated_at();
