-- Create roles table -- this is what a staff wants to or will do
-- and also where chiefs are defined
CREATE TABLE IF NOT EXISTS roles (
    staff UUID NOT NULL,
    atelier UUID NOT NULL,
    validated BOOL NOT NULL,
    chief BOOL NOT NULL,

    PRIMARY KEY (staff, atelier),
    FOREIGN KEY (staff) REFERENCES staff(id) ON DELETE RESTRICT,
    FOREIGN KEY (atelier) REFERENCES ateliers(id) ON DELETE RESTRICT
);
