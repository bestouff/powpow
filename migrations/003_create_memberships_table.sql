-- Create memberships table to track individual registrations/orders
CREATE TABLE IF NOT EXISTS memberships (
    helloasso_order_id BIGINT NOT NULL,
    helloasso_item_id BIGINT NOT NULL,
    payer_email TEXT,

    -- Beneficiary info (the person receiving the membership)
    beneficiary_first_name TEXT,
    beneficiary_last_name TEXT,
    phone TEXT,
    email TEXT,

    -- Order/Item details
    item_name TEXT,
    item_type TEXT,
    tier_name TEXT,
    amount INTEGER,
    order_date TIMESTAMPTZ,
    comment TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (helloasso_item_id),
    FOREIGN KEY (payer_email) REFERENCES users(email) ON DELETE RESTRICT
);

-- Composite type for parameterized queries
DO $$ BEGIN
    CREATE TYPE membership_type AS (
        helloasso_order_id BIGINT,
        helloasso_item_id BIGINT,
        payer_email TEXT,
        beneficiary_first_name TEXT,
        beneficiary_last_name TEXT,
        phone TEXT,
        email TEXT,
        item_name TEXT,
        item_type TEXT,
        tier_name TEXT,
        amount INTEGER,
        order_date TIMESTAMPTZ,
        comment TEXT,
        created_at TIMESTAMPTZ,
        updated_at TIMESTAMPTZ
    );
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
