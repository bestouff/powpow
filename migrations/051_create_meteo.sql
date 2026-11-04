CREATE TABLE meteo (
    station_id TEXT NOT NULL CHECK (station_id ~ '^[0-9]{8}$'),
    observed_at TIMESTAMPTZ NOT NULL,
    reference_time TIMESTAMPTZ NOT NULL,
    insert_time TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    temperature_c DOUBLE PRECISION,
    dew_point_c DOUBLE PRECISION,
    humidity_percent INTEGER,
    wind_direction_deg INTEGER,
    wind_speed_m_s DOUBLE PRECISION,
    gust_direction_deg INTEGER,
    gust_speed_m_s DOUBLE PRECISION,
    rainfall_mm DOUBLE PRECISION,
    snow_depth_m DOUBLE PRECISION,
    pressure_pa DOUBLE PRECISION,
    sea_level_pressure_pa DOUBLE PRECISION,
    raw_data JSONB NOT NULL CHECK (jsonb_typeof(raw_data) = 'object'),
    PRIMARY KEY (station_id, observed_at)
);

COMMENT ON TABLE meteo IS 'Source: Météo-France DPObs v2, six-minute station observations';
COMMENT ON COLUMN meteo.observed_at IS 'Sensor measurement time (validity_time), not polling time';
COMMENT ON COLUMN meteo.reference_time IS 'Météo-France product generation time';
COMMENT ON COLUMN meteo.insert_time IS 'Measurement insertion time in Météo-France databases';
COMMENT ON COLUMN meteo.fetched_at IS 'Time this observation or correction was saved by PowPow';
COMMENT ON COLUMN meteo.temperature_c IS 'Air temperature in degrees Celsius, converted from t in kelvin';
COMMENT ON COLUMN meteo.dew_point_c IS 'Dew point in degrees Celsius, converted from td in kelvin';
COMMENT ON COLUMN meteo.rainfall_mm IS 'Precipitation over the six-minute period (rr_per), in millimetres';
COMMENT ON COLUMN meteo.gust_speed_m_s IS 'Maximum gust over ten minutes (raf10), in metres per second';
COMMENT ON COLUMN meteo.raw_data IS 'Complete API observation in original Météo-France units; unavailable measurements remain null';
