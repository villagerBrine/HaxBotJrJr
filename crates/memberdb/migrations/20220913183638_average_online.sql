-- Add migration script here
ALTER TABLE wynn ADD COLUMN activity_avg INTEGER NOT NULL DEFAULT 0;
ALTER TABLE wynn ADD COLUMN activity_avg_range INTEGER NOT NULL DEFAULT 0;
