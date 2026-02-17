-- Replace photographer_name with photographer_id FK to staff
-- Delete existing photos (they had freetext photographer_name, not linked to staff)
DELETE FROM photos;

-- Drop the old column and index
DROP INDEX IF EXISTS idx_photos_photographer;
ALTER TABLE photos DROP COLUMN photographer_name;

-- Add the new column (NOT NULL, FK to staff)
ALTER TABLE photos ADD COLUMN photographer_id UUID NOT NULL REFERENCES staff(id);

-- Create index on new column
CREATE INDEX idx_photos_photographer_id ON photos(photographer_id);
