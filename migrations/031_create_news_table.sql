CREATE TABLE IF NOT EXISTS news (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    guid TEXT UNIQUE NOT NULL,
    text TEXT NOT NULL,
    link TEXT NOT NULL,
    pub_date TIMESTAMPTZ,
    image_data BYTEA,
    image_mime TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
