-- Cover every identity writer, including automatic and manual imports.
CREATE FUNCTION revoke_old_login_links() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF lower(trim(OLD.email)) IS DISTINCT FROM lower(trim(NEW.email)) THEN
        DELETE FROM login_tokens WHERE staff = NEW.id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER staff_email_login_links
AFTER UPDATE OF email ON staff
FOR EACH ROW EXECUTE FUNCTION revoke_old_login_links();
