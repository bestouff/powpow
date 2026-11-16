ALTER TABLE ateliers RENAME TO poles;
ALTER TABLE roles RENAME COLUMN atelier TO pole;
ALTER TABLE needs RENAME COLUMN atelier TO pole;

-- Constraint renames also rename their backing indexes. Discover the names so
-- this includes the named NOT NULL constraints introduced by PostgreSQL 18.
DO $$
DECLARE
    item record;
BEGIN
    FOR item IN
        SELECT conrelid::regclass AS relation, conname
        FROM pg_constraint
        WHERE conrelid IN ('poles'::regclass, 'roles'::regclass, 'needs'::regclass)
          AND conname LIKE '%atelier%'
    LOOP
        EXECUTE format('ALTER TABLE %s RENAME CONSTRAINT %I TO %I',
                       item.relation, item.conname, replace(item.conname, 'atelier', 'pole'));
    END LOOP;
END;
$$;

ALTER INDEX ateliers_name_key RENAME TO poles_name_key;
-- Index column labels are separate catalog entries; PostgreSQL uses ALTER TABLE
-- for renaming them even though these relations are indexes.
ALTER TABLE roles_pkey RENAME COLUMN atelier TO pole;
ALTER TABLE needs_day_pole_key RENAME COLUMN atelier TO pole;
ALTER TRIGGER ateliers_calendar_updated ON poles RENAME TO poles_calendar_updated;

-- PostgreSQL updates foreign keys and trigger WHEN expressions on column rename,
-- but PL/pgSQL function bodies still refer to the old names as text.
CREATE OR REPLACE FUNCTION remove_ineligible_presence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR (OLD.validated AND NOT NEW.validated) THEN
        PERFORM id FROM staff WHERE id = OLD.staff FOR UPDATE;
        DELETE FROM presence p USING needs n
        WHERE p.needs = n.id AND p.staff = OLD.staff
          AND n.pole = OLD.pole AND n.day >= CURRENT_DATE;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;
