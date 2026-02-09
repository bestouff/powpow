-- Create users table with email as primary key
CREATE TABLE IF NOT EXISTS users (
    email TEXT PRIMARY KEY NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    phone TEXT,
    address TEXT,
    city TEXT,
    zip_code TEXT,
    country TEXT,
    birth_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_sync_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_users_last_sync ON users(last_sync_at);

-- Composite type for parameterized queries
DO $$ BEGIN
    CREATE TYPE user_type AS (
        email TEXT,
        first_name TEXT,
        last_name TEXT,
        phone TEXT,
        address TEXT,
        city TEXT,
        zip_code TEXT,
        country TEXT,
        birth_date TIMESTAMPTZ,
        created_at TIMESTAMPTZ,
        updated_at TIMESTAMPTZ,
        last_sync_at TIMESTAMPTZ
    );
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
