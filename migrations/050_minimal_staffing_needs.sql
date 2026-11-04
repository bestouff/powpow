-- Existing quantities remain the comfortable targets.
ALTER TABLE ateliers ADD COLUMN opening_day_minimal_needed SMALLINT NOT NULL DEFAULT 0;
UPDATE ateliers SET opening_day_minimal_needed = opening_day_typical_needed;
ALTER TABLE ateliers ADD CONSTRAINT ateliers_staffing_thresholds
    CHECK (opening_day_minimal_needed >= 0 AND opening_day_typical_needed >= opening_day_minimal_needed);

ALTER TABLE needs ADD COLUMN minimal_quantity SMALLINT NOT NULL DEFAULT 0;
UPDATE needs SET minimal_quantity = quantity;
ALTER TABLE needs ADD CONSTRAINT needs_staffing_thresholds
    CHECK (minimal_quantity >= 0 AND quantity >= minimal_quantity);
