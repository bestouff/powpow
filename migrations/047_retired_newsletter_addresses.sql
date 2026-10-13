CREATE TABLE mailchimp_retired_emails (email text PRIMARY KEY);

CREATE FUNCTION retire_newsletter_address() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR lower(trim(OLD.email)) IS DISTINCT FROM lower(trim(NEW.email)) THEN
        INSERT INTO mailchimp_retired_emails (email) SELECT lower(trim(OLD.email))
        WHERE trim(OLD.email) <> '' ON CONFLICT DO NOTHING;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER staff_retire_newsletter_address
AFTER DELETE OR UPDATE OF email ON staff
FOR EACH ROW EXECUTE FUNCTION retire_newsletter_address();
