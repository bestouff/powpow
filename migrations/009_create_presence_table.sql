-- Create presence table -- this is where staff announce they
-- are available for an atelier, on a specific day
CREATE TABLE IF NOT EXISTS presence (
    needs UUID NOT NULL,
    staff UUID NOT NULL,
    first_half BOOL NOT NULL,
    second_half BOOL NOT NULL,
    CHECK (first_half OR second_half),

    PRIMARY KEY (needs, staff),
    FOREIGN KEY (needs) REFERENCES needs(id) ON DELETE CASCADE,
    FOREIGN KEY (staff) REFERENCES staff(id) ON DELETE RESTRICT
);
