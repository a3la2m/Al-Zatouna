-- migration 0003: add teacher_id to users for data isolation (FR-4.4)
ALTER TABLE users ADD COLUMN teacher_id INTEGER REFERENCES teachers(id) ON DELETE SET NULL;
CREATE INDEX idx_users_teacher ON users (teacher_id);
