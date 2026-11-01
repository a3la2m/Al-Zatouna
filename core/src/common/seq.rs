//! Central sequential number allocation for vouchers/journal entries.
//!
//! Rule (ADR-010): numbers are minted INSIDE the caller's transaction using
//! an atomic `UPDATE ... RETURNING`; if the transaction rolls back, the
//! counter change unpersists too, so no gaps appear and no number is ever
//! reused.

use sqlx::Row;
use sqlx::SqliteConnection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceKind {
    ReceiptVoucher,
    PaymentVoucher,
    JournalEntry,
    StudentNo,
    TeacherNo,
    CourseNo,
}

impl SequenceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SequenceKind::ReceiptVoucher => "receipt_voucher",
            SequenceKind::PaymentVoucher => "payment_voucher",
            SequenceKind::JournalEntry => "journal_entry",
            SequenceKind::StudentNo => "student_no",
            SequenceKind::TeacherNo => "teacher_no",
            SequenceKind::CourseNo => "course_no",
        }
    }
}

/// Allocate the next number for (branch, kind, fiscal-year) inside the
/// current transaction. `fiscal_year` defaults to the current year for
/// vouchers; use 0 when a sequence is not year-scoped (global kinds).
pub async fn allocate_next_no(
    conn: &mut SqliteConnection,
    branch_id: i64,
    kind: SequenceKind,
    fiscal_year: i64,
) -> crate::error::Result<i64> {
    let kind_s = kind.as_str();
    // Row must be seeded; missing seed is a configuration error.
    let row = sqlx::query(
        r#"
        UPDATE voucher_sequences
           SET last_no = last_no + 1
         WHERE branch_id = ? AND kind = ? AND fiscal_year = ?
         RETURNING last_no
        "#,
    )
    .bind(branch_id)
    .bind(kind_s)
    .bind(fiscal_year)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => crate::error::AppError::conflict(format!(
            "sequence not seeded for branch {branch_id} kind {kind_s} year {fiscal_year}"
        )),
        other => other.into(),
    })?;
    Ok(row.get::<i64, _>(0))
}

/// Seed (or keep existing) a sequence row; safe to call during bootstrap.
pub async fn seed_sequence(
    conn: &mut SqliteConnection,
    branch_id: i64,
    kind: SequenceKind,
    fiscal_year: i64,
) -> crate::error::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO voucher_sequences (branch_id, kind, fiscal_year, last_no)
        VALUES (?, ?, ?, 0)
        ON CONFLICT (branch_id, kind, fiscal_year) DO NOTHING
        "#,
    )
    .bind(branch_id)
    .bind(kind.as_str())
    .bind(fiscal_year)
    .execute(&mut *conn)
    .await
    .map_err(crate::error::AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    #[tokio::test]
    async fn allocation_inside_rolled_back_transaction_leaves_no_gap() {
        let (pool, _guard, branch_id) = database::test_utils::fresh_seeded_pool().await;

        // seed within a closed scope — the 1-connection pool must stay free
        {
            let mut conn = pool.acquire().await.unwrap();
            database::test_utils::seed_branch_sequences(&mut conn, branch_id).await;
        }

        database::test_utils::in_tx(&pool, |mut tx| async move {
            let n1 = allocate_next_no(&mut tx, branch_id, SequenceKind::ReceiptVoucher, 2026)
                .await
                .unwrap();
            assert_eq!(n1, 1);
            let n2 = allocate_next_no(&mut tx, branch_id, SequenceKind::ReceiptVoucher, 2026)
                .await
                .unwrap();
            assert_eq!(n2, 2);
            Err::<(), _>(std::io::Error::other("rollback"))
        })
        .await
        .unwrap_err(); // intended rollback

        // after rollback the counter must be back at zero; the first REAL
        // allocation must yield 1 again (no gap, no lost number)
        let mut tx = pool.begin().await.unwrap();
        let n = allocate_next_no(&mut tx, branch_id, SequenceKind::ReceiptVoucher, 2026)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(n, 1);

        let last: i64 = sqlx::query_scalar(
            "SELECT last_no FROM voucher_sequences
             WHERE kind='receipt_voucher' AND fiscal_year=2026 AND branch_id=?",
        )
        .bind(branch_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(last, 1);
    }
}
