-- Pending addresses live only in encrypted confirmation links, never here.
CREATE TABLE email_change_tokens (
    token uuid PRIMARY KEY,
    staff uuid NOT NULL UNIQUE REFERENCES staff(id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
