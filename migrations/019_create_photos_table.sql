-- Create photos table for storing uploaded photos
CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    photo_data BYTEA NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    photographer_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create index for faster photo lookup
CREATE INDEX idx_photos_photographer ON photos(photographer_name);