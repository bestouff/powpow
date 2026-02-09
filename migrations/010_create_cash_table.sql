-- Create cash payments table -- for tracking cash payments outside HelloAsso
CREATE TABLE IF NOT EXISTS cash (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    email TEXT,
    phone TEXT,
    date DATE NOT NULL,
    amount INTEGER NOT NULL,
    is_membership BOOL NOT NULL
);
