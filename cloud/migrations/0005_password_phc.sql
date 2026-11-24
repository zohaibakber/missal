ALTER TABLE users ADD COLUMN password TEXT NOT NULL DEFAULT '';
UPDATE users SET password = '$pbkdf2-sha256$i=' || password_iterations || ',l=32$'
    || replace(replace(password_salt, '-', '+'), '_', '/') || '$'
    || replace(replace(password_hash, '-', '+'), '_', '/');
ALTER TABLE users DROP COLUMN password_hash;
ALTER TABLE users DROP COLUMN password_salt;
ALTER TABLE users DROP COLUMN password_iterations;
