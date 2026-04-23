//! Integration test: single-table asc -> desc migration end-to-end.
//!
//! Exercises the §7.1 playbook of the descending-sort IDs plan against a
//! live Postgres: preparation, backfill, concurrent index build, NOT NULL
//! fast-path, atomic cutover. Gated on `DATABASE_URL` — returns early if
//! the env var is unset, so the test is a no-op on hosts without a DB.
//!
//! The install helpers (`install_schema`, `install_all_desc_support`,
//! `install_autofill_trigger_for_table`) take a
//! `tokio_postgres::GenericClient`, while the migration itself runs
//! through an `sqlx::PgPool`. We dual-connect: a `tokio_postgres::Client`
//! for the one-shot DDL helpers, and an independent `sqlx::PgPool` for
//! everything else. Isolation is table-based (unique name,
//! `DROP TABLE IF EXISTS ... CASCADE`) so the test can be re-run against
//! the same DB without manual cleanup.
//!
//! Run:
//!
//! ```text
//! DATABASE_URL=postgres://... cargo test -p heeranjid-sqlx \
//!     --test migrate_asc_to_desc -- --test-threads=1
//! ```
//!
//! Task 12 of the v0.3.0 descending-sort IDs plan.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, install_all_desc_support, install_autofill_trigger_for_table,
    install_schema, seed_default_node,
};
use sqlx::postgres::PgPoolOptions;
use tokio_postgres::NoTls;

/// Table used by this test. Unique to the file so repeated runs don't
/// collide; `DROP TABLE IF EXISTS ... CASCADE` at start + end keeps it
/// idempotent.
const TABLE: &str = "mig_tbl_single_desc";

#[tokio::test(flavor = "multi_thread")]
async fn single_table_migration_end_to_end() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live migration test");
        return;
    };

    // --- Dual-connect: tokio-postgres for install helpers, sqlx for the rest ---
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("sqlx connect");

    let (pg_client, pg_conn) = tokio_postgres::connect(&url, NoTls)
        .await
        .expect("tokio-postgres connect");
    tokio::spawn(async move {
        if let Err(e) = pg_conn.await {
            eprintln!("postgres connection error: {e}");
        }
    });

    // --- Install base schema + desc support (idempotent) ---
    install_schema(&pg_client).await.expect("install_schema");
    seed_default_node(&pg_client)
        .await
        .expect("seed_default_node");
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");

    // heerid_next() reads the session's `heer.node_id`; set it for both
    // the tokio-postgres client (used for trigger install DDL) and the
    // sqlx pool connections (used for INSERTs via DEFAULT heerid_next()).
    pg_client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set_heer_node_id on tokio-postgres client");

    // --- Fixture ---
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE} CASCADE"))
        .execute(&pool)
        .await
        .expect("drop pre-existing fixture");
    sqlx::query(&format!(
        "CREATE TABLE {TABLE} (id bigint PRIMARY KEY DEFAULT heerid_next())"
    ))
    .execute(&pool)
    .await
    .expect("create fixture");

    // Use one pinned sqlx connection for the fixture INSERT so the
    // `set_heer_node_id(1)` session setting persists across statements.
    let mut fixture_conn = pool.acquire().await.expect("acquire fixture conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *fixture_conn)
        .await
        .expect("set_heer_node_id on fixture conn");
    sqlx::query(&format!(
        "INSERT INTO {TABLE} SELECT heerid_next() FROM generate_series(1, 10000)"
    ))
    .execute(&mut *fixture_conn)
    .await
    .expect("seed 10k rows");
    drop(fixture_conn);

    let row_count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {TABLE}"))
        .fetch_one(&pool)
        .await
        .expect("row count");
    assert_eq!(row_count, 10_000, "fixture should have 10k rows");

    // --- Phase 1: preparation — add sibling column + per-table trigger ---
    sqlx::query(&format!("ALTER TABLE {TABLE} ADD COLUMN id_desc bigint"))
        .execute(&pool)
        .await
        .expect("add id_desc column");

    install_autofill_trigger_for_table(
        &pg_client,
        TABLE,
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install_autofill_trigger_for_table");

    // --- Phase 2: backfill (top-level CALL, not inside a transaction) ---
    sqlx::query(&format!(
        "CALL heeranjid_bulk_backfill('{TABLE}','id','id_desc','heer',2000)"
    ))
    .execute(&pool)
    .await
    .expect("bulk backfill");

    // Assert completeness for non-nullable source: every row has id_desc.
    let missing: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {TABLE} WHERE id_desc IS NULL"
    ))
    .fetch_one(&pool)
    .await
    .expect("count NULL id_desc");
    assert_eq!(missing, 0, "no NULL id_desc after backfill");

    // Assert backfill correctness: id_desc == heerid_to_desc(id) for all rows.
    let wrong: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {TABLE} WHERE id_desc <> heerid_to_desc(id)"
    ))
    .fetch_one(&pool)
    .await
    .expect("count divergent rows");
    assert_eq!(wrong, 0, "id_desc must equal heerid_to_desc(id) everywhere");

    // --- Phase 3: index build (outside txn) + NOT NULL fast path ---
    let idx = format!("idx_{TABLE}_id_desc");
    let check = format!("{TABLE}_id_desc_nn");
    sqlx::query(&format!(
        "CREATE UNIQUE INDEX CONCURRENTLY {idx} ON {TABLE} (id_desc)"
    ))
    .execute(&pool)
    .await
    .expect("concurrent unique index build");
    sqlx::query(&format!(
        "ALTER TABLE {TABLE} ADD CONSTRAINT {check} CHECK (id_desc IS NOT NULL) NOT VALID"
    ))
    .execute(&pool)
    .await
    .expect("add NOT VALID check");
    sqlx::query(&format!("ALTER TABLE {TABLE} VALIDATE CONSTRAINT {check}"))
        .execute(&pool)
        .await
        .expect("validate check");
    sqlx::query(&format!(
        "ALTER TABLE {TABLE} ALTER COLUMN id_desc SET NOT NULL"
    ))
    .execute(&pool)
    .await
    .expect("set NOT NULL");
    sqlx::query(&format!("ALTER TABLE {TABLE} DROP CONSTRAINT {check}"))
        .execute(&pool)
        .await
        .expect("drop redundant check");

    // --- Phase 4: atomic cutover ---
    let mut tx = pool.begin().await.expect("begin cutover tx");
    sqlx::query(&format!("ALTER TABLE {TABLE} DROP CONSTRAINT {TABLE}_pkey"))
        .execute(&mut *tx)
        .await
        .expect("drop old pkey");
    sqlx::query(&format!(
        "ALTER TABLE {TABLE} ADD CONSTRAINT {TABLE}_pkey PRIMARY KEY USING INDEX {idx}"
    ))
    .execute(&mut *tx)
    .await
    .expect("promote unique index to pkey");
    sqlx::query(&format!(
        "ALTER TABLE {TABLE} ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()"
    ))
    .execute(&mut *tx)
    .await
    .expect("set desc DEFAULT on id_desc");
    sqlx::query(&format!("ALTER TABLE {TABLE} ALTER COLUMN id DROP DEFAULT"))
        .execute(&mut *tx)
        .await
        .expect("drop DEFAULT on id");
    sqlx::query(&format!("ALTER TABLE {TABLE} DROP COLUMN id"))
        .execute(&mut *tx)
        .await
        .expect("drop old id column");
    sqlx::query(&format!(
        "DROP TRIGGER zzz_{TABLE}_autofill_desc ON {TABLE}"
    ))
    .execute(&mut *tx)
    .await
    .expect("drop autofill trigger");
    sqlx::query(&format!(
        "DROP FUNCTION zzz_{TABLE}_autofill_desc() CASCADE"
    ))
    .execute(&mut *tx)
    .await
    .expect("drop autofill trigger fn");
    sqlx::query(&format!("ALTER TABLE {TABLE} RENAME COLUMN id_desc TO id"))
        .execute(&mut *tx)
        .await
        .expect("rename id_desc -> id");
    tx.commit().await.expect("commit cutover");

    // --- Post-cutover assertions ---
    // (a) ORDER BY id ASC returns rows in reverse-chronological logical
    //     order, because `id` now stores the descending flip.
    let ids: Vec<i64> =
        sqlx::query_scalar(&format!("SELECT id FROM {TABLE} ORDER BY id LIMIT 100"))
            .fetch_all(&pool)
            .await
            .expect("fetch sorted ids");
    assert_eq!(ids.len(), 100, "expected 100 rows sampled");
    let logical: Vec<u64> = ids
        .iter()
        .map(|&raw| {
            heeranjid::HeerIdDesc::from_i64(raw)
                .expect("stored value is a valid HeerIdDesc")
                .timestamp_ms()
        })
        .collect();
    assert!(
        logical.windows(2).all(|w| w[0] >= w[1]),
        "ORDER BY id must yield reverse-chronological rows: {logical:?}"
    );

    // (b) The old (ascending) `id` column is gone — only the renamed
    //     `id_desc -> id` remains.
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_name = $1 ORDER BY column_name",
    )
    .bind(TABLE)
    .fetch_all(&pool)
    .await
    .expect("list columns");
    assert_eq!(
        cols,
        vec!["id".to_string()],
        "after cutover, only the renamed `id` column should remain"
    );

    // (c) The primary key is intact and backed by the promoted index.
    let pk_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_index i \
         JOIN pg_class c ON c.oid = i.indrelid \
         WHERE c.relname = $1 AND i.indisprimary",
    )
    .bind(TABLE)
    .fetch_one(&pool)
    .await
    .expect("count primary keys");
    assert_eq!(pk_count, 1, "table should still have exactly one PK");

    // --- Cleanup ---
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE} CASCADE"))
        .execute(&pool)
        .await
        .expect("final cleanup drop");
}
