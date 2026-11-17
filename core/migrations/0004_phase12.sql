-- payment_applications: link a single receipt payment to multiple enrollments
CREATE TABLE IF NOT EXISTS payment_applications (
    id             INTEGER PRIMARY KEY,
    receipt_voucher_id INTEGER NOT NULL REFERENCES receipt_vouchers(id) ON DELETE CASCADE,
    enrollment_id  INTEGER NOT NULL,
    amount_minor   INTEGER NOT NULL CHECK (amount_minor >= 0),
    created_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_payment_applications_receipt ON payment_applications (receipt_voucher_id);
CREATE INDEX IF NOT EXISTS idx_payment_applications_enrollment ON payment_applications (enrollment_id);
