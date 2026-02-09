-- Create ateliers table -- this is where all available jobs are stored
CREATE TABLE IF NOT EXISTS ateliers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    needs_validation BOOL NOT NULL
);
