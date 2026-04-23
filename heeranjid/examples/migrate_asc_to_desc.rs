//! Runnable end-to-end example: migrate a single table from ascending
//! HeerId primary keys to descending HeerId primary keys, live against a
//! Postgres instance pointed at by `DATABASE_URL`. Mirrors the §7.1
//! playbook and the §8.1 spec layout.
//!
//! # How to run
//!
//! ```bash
//! DATABASE_URL=postgres://... \
//!     cargo run -p heeranjid \
//!     --features "sqlx postgres" \
//!     --example migrate_asc_to_desc
//! ```
//!
//! The program exits 0 on a clean migration and prints one banner per
//! phase so operators can watch progress.
//!
//! # Memory characteristics (spec §8.1)
//!
//! - **Rust-side memory: O(1).** No row data ever crosses the sqlx /
//!   tokio-postgres boundary — the entire migration is driven via DDL
//!   statements and a single `CALL heeranjid_bulk_backfill(...)`. The
//!   Rust process holds connection pool buffers and nothing proportional
//!   to table size.
//! - **Postgres-side memory per commit: O(batch_size).** The backfill
//!   procedure commits every `batch_size` rows (2000 below) so the
//!   server never accumulates more than one batch of tuples in a single
//!   transaction's WAL / snapshot window.
//! - **Total Rust memory during the migration: ~the sqlx connection
//!   pool.** Independent of row count; measured and asserted separately
//!   in the integration test suite.
//!
//! # Why each phase uses its particular transaction context
//!
//! 1. **Preparation (bare `&PgPool`).** `ALTER TABLE ADD COLUMN` +
//!    `CREATE TRIGGER` are cheap catalog ops; running them outside an
//!    explicit transaction means they auto-commit immediately and
//!    release their `AccessExclusiveLock` the moment they return.
//! 2. **Backfill (top-level `CALL`, not inside a transaction).**
//!    `heeranjid_bulk_backfill` issues its own `COMMIT`s between
//!    batches. Postgres rejects `CALL` on a procedure that uses
//!    transaction control if it's already inside a BEGIN block, so the
//!    call must be top-level.
//! 3. **Index build (bare `&PgPool`).** `CREATE INDEX CONCURRENTLY` is
//!    explicitly forbidden inside a transaction — it manages its own
//!    snapshot to avoid blocking writers.
//! 4. **Cutover (single `pool.begin()` transaction).** Dropping the old
//!    PK, promoting the new index to PK, swapping defaults, and
//!    dropping the old column must be atomic so that there is no window
//!    in which the table has no primary key or a stale default. All
//!    catalog changes issue before `tx.commit()`.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, install_all_desc_support, install_autofill_trigger_for_table,
    install_schema, seed_default_node,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Two connection handles to the same database:
    //   * `pool`      — sqlx for DDL, CALL, and the cutover transaction.
    //   * `pg_client` — tokio-postgres for the `install_*` helpers,
    //                   which are generic over `GenericClient`.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;
    let (pg_client, pg_conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls).await?;
    tokio::spawn(async move {
        let _ = pg_conn.await;
    });

    // --- Install base schema + desc support (idempotent) ---
    install_schema(&pg_client).await?;
    // `seed_default_node` fails if node_id = 1 already exists; ignore
    // that so the example is re-runnable against a shared DB.
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client).await?;

    // --- Fixture: fresh table with 10k rows under the ascending PK ---
    sqlx::query("DROP TABLE IF EXISTS mig_tbl CASCADE")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE TABLE mig_tbl (id bigint PRIMARY KEY DEFAULT heerid_next())")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO mig_tbl SELECT heerid_next() FROM generate_series(1, 10000)")
        .execute(&pool)
        .await?;

    // ------------------------------------------------------------------
    // Phase 1: preparation
    // ------------------------------------------------------------------
    // Add the descending sibling column (nullable for now — the trigger
    // + backfill will populate it) and install the autofill trigger so
    // any concurrent INSERT/UPDATE in steady state keeps `id_desc` in
    // sync with `id`.
    eprintln!("Phase 1: preparation — add id_desc column + install autofill trigger");
    sqlx::query("ALTER TABLE mig_tbl ADD COLUMN id_desc bigint")
        .execute(&pool)
        .await?;
    install_autofill_trigger_for_table(
        &pg_client,
        "mig_tbl",
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await?;

    // ------------------------------------------------------------------
    // Phase 2: backfill (top-level CALL, not inside a transaction)
    // ------------------------------------------------------------------
    // `heeranjid_bulk_backfill` uses procedure-level COMMITs, so it must
    // not be wrapped in `pool.begin()` — sqlx's plain `execute` runs the
    // statement on an auto-commit connection, which is exactly what the
    // procedure needs.
    eprintln!("Phase 2: backfill — CALL heeranjid_bulk_backfill (batch_size = 2000)");
    sqlx::query("CALL heeranjid_bulk_backfill('mig_tbl','id','id_desc','heer',2000)")
        .execute(&pool)
        .await?;

    let missing: i64 = sqlx::query_scalar("SELECT count(*) FROM mig_tbl WHERE id_desc IS NULL")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        missing, 0,
        "backfill left NULLs behind in id_desc; trigger may be misconfigured"
    );

    // ------------------------------------------------------------------
    // Phase 3: index build (OUTSIDE a transaction) + NOT NULL fast path
    // ------------------------------------------------------------------
    // CREATE INDEX CONCURRENTLY must run outside any transaction block.
    // Once built, we tighten the column to NOT NULL via the CHECK NOT
    // VALID → VALIDATE → SET NOT NULL → DROP CHECK dance: that keeps
    // the AccessExclusiveLock window to a single catalog flip per step
    // rather than a full-table scan while holding the lock.
    eprintln!("Phase 3: index build — CREATE INDEX CONCURRENTLY + NOT NULL fast path");
    sqlx::query("CREATE UNIQUE INDEX CONCURRENTLY idx_mig_tbl_id_desc ON mig_tbl (id_desc)")
        .execute(&pool)
        .await?;
    sqlx::query(
        "ALTER TABLE mig_tbl ADD CONSTRAINT mig_tbl_id_desc_nn \
         CHECK (id_desc IS NOT NULL) NOT VALID",
    )
    .execute(&pool)
    .await?;
    sqlx::query("ALTER TABLE mig_tbl VALIDATE CONSTRAINT mig_tbl_id_desc_nn")
        .execute(&pool)
        .await?;
    sqlx::query("ALTER TABLE mig_tbl ALTER COLUMN id_desc SET NOT NULL")
        .execute(&pool)
        .await?;
    sqlx::query("ALTER TABLE mig_tbl DROP CONSTRAINT mig_tbl_id_desc_nn")
        .execute(&pool)
        .await?;

    // ------------------------------------------------------------------
    // Phase 4: atomic cutover
    // ------------------------------------------------------------------
    // Everything from here until tx.commit() is one atomic block so
    // there is never an observable moment where mig_tbl has no primary
    // key or a stale default.
    eprintln!("Phase 4: cutover — swap PK, swap DEFAULT, drop old column, rename");
    let mut tx = pool.begin().await?;
    sqlx::query("ALTER TABLE mig_tbl DROP CONSTRAINT mig_tbl_pkey")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "ALTER TABLE mig_tbl ADD CONSTRAINT mig_tbl_pkey \
         PRIMARY KEY USING INDEX idx_mig_tbl_id_desc",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("ALTER TABLE mig_tbl ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await?;
    sqlx::query("ALTER TABLE mig_tbl ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await?;
    sqlx::query("ALTER TABLE mig_tbl DROP COLUMN id")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP TRIGGER zzz_mig_tbl_autofill_desc ON mig_tbl")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP FUNCTION zzz_mig_tbl_autofill_desc() CASCADE")
        .execute(&mut *tx)
        .await?;
    sqlx::query("ALTER TABLE mig_tbl RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // --- Post-cutover sanity checks ---
    // (a) A plain ASC scan by id must now yield rows in reverse
    //     chronological order (newest first), which is the whole point
    //     of the descending-sort variant.
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM mig_tbl ORDER BY id LIMIT 5")
        .fetch_all(&pool)
        .await?;
    let logical: Vec<u64> = ids
        .iter()
        .map(|&raw| heeranjid::HeerIdDesc::from_i64(raw).unwrap().timestamp_ms())
        .collect();
    assert!(
        logical.windows(2).all(|w| w[0] >= w[1]),
        "ORDER BY id should return reverse-chronological rows: {logical:?}"
    );

    // (b) The old ascending column is gone; only `id` (the descending
    //     one, just renamed) should remain.
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_name = 'mig_tbl'",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(cols, vec!["id".to_string()]);

    // Leave the fixture clean for repeat runs.
    sqlx::query("DROP TABLE mig_tbl").execute(&pool).await?;

    eprintln!("migration complete");
    Ok(())
}
