-- Add must_change_password flag to users.
-- When set to 1 the user must change their password before using the app.
-- Admin-created accounts start with this flag set; self-registered users do not.

ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
