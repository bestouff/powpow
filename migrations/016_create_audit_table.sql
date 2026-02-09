CREATE TABLE IF NOT EXISTS audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    staff_id UUID,
    staff_name TEXT NOT NULL DEFAULT '',
    operation TEXT NOT NULL,
    detail TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_audit_created_at ON audit (created_at DESC);
