-- Create needs table -- this is where we start declaring we need
-- some people for some specific roles on a precise moment
CREATE TABLE IF NOT EXISTS needs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    day DATE NOT NULL,
    atelier UUID NOT NULL,
    quantity SMALLINT NOT NULL CHECK (quantity > 0),
    nightly BOOL NOT NULL,

    UNIQUE (day, atelier),
    FOREIGN KEY (atelier) REFERENCES ateliers(id) ON DELETE RESTRICT
);
