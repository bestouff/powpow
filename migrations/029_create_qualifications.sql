-- Create qualifications table (types of training/certifications)
CREATE TABLE IF NOT EXISTS qualifications (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    duration SMALLINT  -- number of years the qualification is valid, NULL = lifelong
);

-- Create staff_qualif table (records a staff member obtaining a qualification)
CREATE TABLE IF NOT EXISTS staff_qualif (
    id SERIAL PRIMARY KEY,
    staff UUID NOT NULL REFERENCES staff(id) ON DELETE CASCADE,
    qualification INTEGER NOT NULL REFERENCES qualifications(id) ON DELETE CASCADE,
    obtained_date DATE NOT NULL
);

CREATE INDEX idx_staff_qualif_staff ON staff_qualif(staff);
CREATE INDEX idx_staff_qualif_qualification ON staff_qualif(qualification);
