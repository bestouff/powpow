-- Add payment_method column to cash table to support check payments
-- Default to 'cash' for existing records
ALTER TABLE cash ADD COLUMN IF NOT EXISTS payment_method TEXT NOT NULL DEFAULT 'cash';
