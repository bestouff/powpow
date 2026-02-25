CREATE TYPE opening_day_status AS ENUM ('reserved', 'validated', 'canceled');

CREATE TABLE IF NOT EXISTS opening_days (
    day DATE PRIMARY KEY,
    status opening_day_status NOT NULL DEFAULT 'reserved'
);
