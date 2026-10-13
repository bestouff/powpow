-- Role deletion and validation revocation must serialize with planning updates.
CREATE FUNCTION remove_ineligible_presence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR (OLD.validated AND NOT NEW.validated) THEN
        PERFORM id FROM staff WHERE id = OLD.staff FOR UPDATE;
        DELETE FROM presence p USING needs n
        WHERE p.needs = n.id AND p.staff = OLD.staff
          AND n.atelier = OLD.atelier AND n.day >= CURRENT_DATE;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER roles_remove_ineligible_presence
BEFORE DELETE OR UPDATE OF validated ON roles
FOR EACH ROW EXECUTE FUNCTION remove_ineligible_presence();

DELETE FROM presence p USING needs n
WHERE p.needs = n.id AND n.day >= CURRENT_DATE
  AND NOT EXISTS (SELECT 1 FROM roles r WHERE r.staff = p.staff AND r.atelier = n.atelier AND r.validated);
