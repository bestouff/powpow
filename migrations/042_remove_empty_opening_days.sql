DELETE FROM opening_days d
WHERE NOT EXISTS (SELECT 1 FROM needs n WHERE n.day = d.day);

-- Serialize changes to a day's metadata so concurrent deletions of its last
-- needs cannot leave an orphaned opening day.
CREATE FUNCTION remove_empty_opening_day() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.day = OLD.day THEN
        RETURN NULL;
    END IF;

    PERFORM day FROM opening_days WHERE day = OLD.day FOR UPDATE;
    DELETE FROM opening_days d
    WHERE d.day = OLD.day
      AND NOT EXISTS (SELECT 1 FROM needs n WHERE n.day = d.day);
    RETURN NULL;
END;
$$;

CREATE TRIGGER remove_empty_opening_day
AFTER DELETE OR UPDATE OF day ON needs
FOR EACH ROW EXECUTE FUNCTION remove_empty_opening_day();

-- Creation inserts the opening day and its needs in one transaction. Check
-- at commit, allowing either insertion order within that transaction.
CREATE FUNCTION check_opening_day_has_needs() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM opening_days WHERE day = NEW.day)
       AND NOT EXISTS (SELECT 1 FROM needs WHERE day = NEW.day) THEN
        RAISE EXCEPTION 'Opening day % must have at least one need', NEW.day
            USING ERRCODE = '23514';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER opening_day_has_needs
AFTER INSERT OR UPDATE ON opening_days
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION check_opening_day_has_needs();

CREATE FUNCTION remove_opening_days_after_truncate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM opening_days;
    RETURN NULL;
END;
$$;

CREATE TRIGGER remove_opening_days_after_truncate
AFTER TRUNCATE ON needs
FOR EACH STATEMENT EXECUTE FUNCTION remove_opening_days_after_truncate();
