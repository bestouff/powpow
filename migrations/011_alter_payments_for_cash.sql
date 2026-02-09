-- Alter payments table to support cash payments alongside HelloAsso
-- Add UUID primary key, make helloasso_item_id nullable, add cash_id with XOR constraint

-- Add new id column
ALTER TABLE payments ADD COLUMN IF NOT EXISTS id UUID DEFAULT uuid_generate_v4();

-- Add cash_id column
ALTER TABLE payments ADD COLUMN IF NOT EXISTS cash_id UUID;

-- Drop old primary key (helloasso_item_id)
ALTER TABLE payments DROP CONSTRAINT IF EXISTS payments_pkey;

-- Allow helloasso_item_id to be NULL (for cash payments)
ALTER TABLE payments ALTER COLUMN helloasso_item_id DROP NOT NULL;

-- Set id as primary key (fill in any NULLs first)
UPDATE payments SET id = uuid_generate_v4() WHERE id IS NULL;
ALTER TABLE payments ALTER COLUMN id SET NOT NULL;

-- Add primary key on id (idempotent with DO block)
DO $$ BEGIN
    ALTER TABLE payments ADD PRIMARY KEY (id);
EXCEPTION WHEN duplicate_table THEN NULL;
END $$;

-- Add UNIQUE constraints (idempotent with DO block)
DO $$ BEGIN
    ALTER TABLE payments ADD CONSTRAINT payments_helloasso_item_id_key UNIQUE (helloasso_item_id);
EXCEPTION WHEN duplicate_table OR duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE payments ADD CONSTRAINT payments_cash_id_key UNIQUE (cash_id);
EXCEPTION WHEN duplicate_table OR duplicate_object THEN NULL;
END $$;

-- Add XOR check: exactly one of helloasso_item_id or cash_id must be set
DO $$ BEGIN
    ALTER TABLE payments ADD CONSTRAINT payments_source_xor CHECK ((helloasso_item_id IS NOT NULL) != (cash_id IS NOT NULL));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- Add foreign key to cash table
DO $$ BEGIN
    ALTER TABLE payments ADD CONSTRAINT payments_cash_id_fkey FOREIGN KEY (cash_id) REFERENCES cash(id);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
