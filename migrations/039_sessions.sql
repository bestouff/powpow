-- Replace single per-staff login token with proper sessions + login tokens
-- to allow multiple simultaneous sessions per user.

ALTER TABLE staff DROP COLUMN IF EXISTS token;

CREATE TABLE login_tokens (
    token      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    staff      UUID NOT NULL REFERENCES staff(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX login_tokens_staff_idx ON login_tokens (staff);

CREATE TABLE sessions (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    staff      UUID NOT NULL REFERENCES staff(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    user_agent TEXT,
    ip         TEXT
);

CREATE INDEX sessions_staff_idx ON sessions (staff);