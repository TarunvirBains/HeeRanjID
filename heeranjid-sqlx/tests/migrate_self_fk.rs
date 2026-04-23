//! Self-referential FK asc→desc migration (spec §7.4).
//!
//! Exercises the playbook from `docs/migrations/asc-to-desc.md` against a
//! live Postgres for the tricky case where a single table has both a PK
//! and a nullable self-referencing FK. Both columns migrate to their
//! descending siblings in one go using a **single** autofill trigger
//! installed with two `ColumnPair` entries — the multi-pair trigger
//! called out in spec §5.1 (the `zzz_nodes_autofill_desc` example).
//!
//! # Why this is the interesting case
//!
//! * One trigger, two pairs: `id → id_desc` and `parent_id → parent_id_desc`
//!   are kept in sync by the same function, proving the multi-pair helper
//!   handles updates to either source column (including FK cascades
//!   on the same row).
//! * Null tracking: `parent_id` is nullable, so the verification uses the
//!   `IS DISTINCT FROM` / `heerid_to_desc(parent_id)` invariant from
//!   spec §7.1 step 4 rather than a blind equality check.
//! * Self-FK cutover: the new FK references `id_desc` (the column that
//!   *becomes* the PK), not the pre-rename `id`. This is why the DROP /
//!   promote / re-add happens inside one atomic transaction.
//!
//! Mirrors the conventions in `migrate_asc_to_desc_with_fk.rs`:
//! dual-connect (sqlx pool + tokio-postgres client), `DATABASE_URL`
//! gating, pinned fixture connection for `set_heer_node_id(1)`, unique
//! table name with DROP-IF-EXISTS bookends for re-runnability.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, drop_autofill_trigger_for_table, install_all_desc_support,
    install_autofill_trigger_for_table, install_schema, seed_default_node,
};
use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;

/// Spawn a dedicated `tokio_postgres::Client` alongside the sqlx pool.
async fn dual_connect(url: &str) -> tokio_postgres::Client {
    let (client, conn) = tokio_postgres::connect(url, tokio_postgres::NoTls)
        .await
        .expect("tokio-postgres connect");
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("tokio-postgres connection error: {e}");
        }
    });
    client
}

#[tokio::test(flavor = "multi_thread")]
async fn self_fk_migration_end_to_end() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect sqlx pool");
    let pg_client = dual_connect(&url).await;

    // --- Base schema + desc support (idempotent) ---
    install_schema(&pg_client).await.expect("install_schema");
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");

    // --- Fixture: unique table name, DROP IF EXISTS for re-runnability ---
    pool.execute("DROP TABLE IF EXISTS nodes_self_fk CASCADE")
        .await
        .unwrap();

    pool.execute(
        "CREATE TABLE nodes_self_fk (
            id bigint PRIMARY KEY DEFAULT heerid_next(),
            parent_id bigint NULL REFERENCES nodes_self_fk(id)
         )",
    )
    .await
    .unwrap();

    // Pin a sqlx connection so `set_heer_node_id(1)` persists across the
    // `heerid_next()` calls inside the seed INSERTs.
    let mut fixture_conn = pool.acquire().await.expect("acquire fixture conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *fixture_conn)
        .await
        .expect("set_heer_node_id on fixture conn");

    // 10 root nodes (parent_id IS NULL).
    sqlx::query(
        "INSERT INTO nodes_self_fk (id, parent_id)
         SELECT heerid_next(), NULL FROM generate_series(1, 10)",
    )
    .execute(&mut *fixture_conn)
    .await
    .unwrap();

    // 90 child nodes, round-robin across the 10 roots. We order the roots
    // deterministically and use `row_number() - 1` as the 0..10 index, then
    // attach each child to root (gs % 10).
    sqlx::query(
        "WITH roots AS (
             SELECT id, row_number() OVER (ORDER BY id) - 1 AS idx
             FROM nodes_self_fk
             WHERE parent_id IS NULL
         )
         INSERT INTO nodes_self_fk (id, parent_id)
         SELECT heerid_next(), r.id
         FROM generate_series(0, 89) AS g
         JOIN roots r ON r.idx = (g % 10)",
    )
    .execute(&mut *fixture_conn)
    .await
    .unwrap();
    drop(fixture_conn);

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM nodes_self_fk")
        .fetch_one(&pool)
        .await
        .unwrap();
    let roots: i64 =
        sqlx::query_scalar("SELECT count(*) FROM nodes_self_fk WHERE parent_id IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total, 100, "fixture seed total");
    assert_eq!(roots, 10, "fixture seed roots");

    // --- Phase 1: preparation — add desc columns + install one multi-pair trigger ---
    pool.execute("ALTER TABLE nodes_self_fk ADD COLUMN id_desc bigint")
        .await
        .unwrap();
    pool.execute("ALTER TABLE nodes_self_fk ADD COLUMN parent_id_desc bigint")
        .await
        .unwrap();

    // ONE install call with TWO pairs → one function, one trigger, handling
    // updates to either source column (spec §5.1).
    install_autofill_trigger_for_table(
        &pg_client,
        "nodes_self_fk",
        &[
            ColumnPair {
                src: "id",
                dst: "id_desc",
            },
            ColumnPair {
                src: "parent_id",
                dst: "parent_id_desc",
            },
        ],
        IdKind::Heer,
    )
    .await
    .expect("install multi-pair trigger on nodes_self_fk");

    // Confirm the generated function is named exactly as the spec's
    // §5.1 worked example predicts (`zzz_<table>_autofill_desc`). This
    // is what validates the helper's naming convention against a
    // multi-word table name ("nodes_self_fk").
    let trig_fn_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM pg_proc
             WHERE proname = 'zzz_nodes_self_fk_autofill_desc'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        trig_fn_exists,
        "expected generated function zzz_nodes_self_fk_autofill_desc to exist"
    );

    // --- Phase 2: backfill (two top-level CALLs, procedure manages its own tx) ---
    pool.execute("CALL heeranjid_bulk_backfill('nodes_self_fk','id','id_desc','heer',50)")
        .await
        .unwrap();
    pool.execute(
        "CALL heeranjid_bulk_backfill('nodes_self_fk','parent_id','parent_id_desc','heer',50)",
    )
    .await
    .unwrap();

    // --- Phase 3: verification (null-tracking invariant, spec §7.1 step 4) ---
    let id_mismatches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM nodes_self_fk WHERE id_desc <> heerid_to_desc(id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(id_mismatches, 0, "id → id_desc flip invariant");

    let parent_mismatches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM nodes_self_fk
         WHERE (parent_id IS NULL) IS DISTINCT FROM (parent_id_desc IS NULL)
            OR (parent_id IS NOT NULL AND parent_id_desc <> heerid_to_desc(parent_id))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        parent_mismatches, 0,
        "parent_id → parent_id_desc flip invariant (null-tracking)"
    );

    // --- Phase 4: indexes + NOT NULL fast-path (outside any transaction) ---
    pool.execute(
        "CREATE UNIQUE INDEX CONCURRENTLY idx_nodes_self_fk_id_desc
         ON nodes_self_fk (id_desc)",
    )
    .await
    .unwrap();
    // FK column is nullable → non-unique index is sufficient.
    pool.execute(
        "CREATE INDEX CONCURRENTLY idx_nodes_self_fk_parent_id_desc
         ON nodes_self_fk (parent_id_desc)",
    )
    .await
    .unwrap();

    // NOT NULL fast-path for id_desc only (parent_id is nullable, so
    // parent_id_desc stays nullable).
    pool.execute(
        "ALTER TABLE nodes_self_fk
         ADD CONSTRAINT nodes_self_fk_id_desc_nn CHECK (id_desc IS NOT NULL) NOT VALID",
    )
    .await
    .unwrap();
    pool.execute("ALTER TABLE nodes_self_fk VALIDATE CONSTRAINT nodes_self_fk_id_desc_nn")
        .await
        .unwrap();
    pool.execute("ALTER TABLE nodes_self_fk ALTER COLUMN id_desc SET NOT NULL")
        .await
        .unwrap();
    pool.execute("ALTER TABLE nodes_self_fk DROP CONSTRAINT nodes_self_fk_id_desc_nn")
        .await
        .unwrap();

    // --- Phase 5: atomic cutover — ONE transaction ---
    //
    // Ordering (spec §7.4 applied to self-FK):
    //   1. Drop old self-FK.
    //   2. Drop old PK.
    //   3. Promote idx_nodes_self_fk_id_desc to the new PK.
    //   4. Add new self-FK NOT VALID (references id_desc, still named).
    //   5. Swap defaults + drop old columns + drop trigger/function.
    //   6. Rename new columns into place.
    let mut tx = pool.begin().await.unwrap();

    // (1) Drop old self-FK. Default auto-generated name is
    //     `<table>_<col>_fkey` since we control the CREATE.
    sqlx::query("ALTER TABLE nodes_self_fk DROP CONSTRAINT nodes_self_fk_parent_id_fkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (2) Drop old PK.
    sqlx::query("ALTER TABLE nodes_self_fk DROP CONSTRAINT nodes_self_fk_pkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (3) Promote desc index to PK.
    sqlx::query(
        "ALTER TABLE nodes_self_fk
         ADD CONSTRAINT nodes_self_fk_pkey
         PRIMARY KEY USING INDEX idx_nodes_self_fk_id_desc",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (4) Add new self-FK as NOT VALID, references the still-named id_desc.
    sqlx::query(
        "ALTER TABLE nodes_self_fk
         ADD CONSTRAINT nodes_self_fk_parent_id_desc_fkey
         FOREIGN KEY (parent_id_desc) REFERENCES nodes_self_fk(id_desc) NOT VALID",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (5) Defaults, drop old columns, drop trigger + function.
    sqlx::query("ALTER TABLE nodes_self_fk ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE nodes_self_fk ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE nodes_self_fk DROP COLUMN id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE nodes_self_fk DROP COLUMN parent_id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER zzz_nodes_self_fk_autofill_desc ON nodes_self_fk")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION zzz_nodes_self_fk_autofill_desc() CASCADE")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (6) Rename both new columns into their final names.
    sqlx::query("ALTER TABLE nodes_self_fk RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE nodes_self_fk RENAME COLUMN parent_id_desc TO parent_id")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // --- Phase 6: validate deferred FK outside the cutover tx ---
    pool.execute("ALTER TABLE nodes_self_fk VALIDATE CONSTRAINT nodes_self_fk_parent_id_desc_fkey")
        .await
        .unwrap();

    // --- Assertions ---

    // (a) Row count preserved.
    let post_total: i64 = sqlx::query_scalar("SELECT count(*) FROM nodes_self_fk")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(post_total, 100, "total row count preserved");

    // (b) Self-FK integrity: every child points at a real parent.
    let orphans: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM nodes_self_fk c
         LEFT JOIN nodes_self_fk p ON c.parent_id = p.id
         WHERE c.parent_id IS NOT NULL AND p.id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphans, 0, "no orphaned children after self-FK migration");

    // (c) Exactly 10 roots survived.
    let post_roots: i64 =
        sqlx::query_scalar("SELECT count(*) FROM nodes_self_fk WHERE parent_id IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(post_roots, 10, "exactly 10 roots preserved");

    // (d) ORDER BY id returns reverse-chronological rows (descending sort).
    let sample_ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM nodes_self_fk ORDER BY id LIMIT 10")
            .fetch_all(&pool)
            .await
            .unwrap();
    let sample_ts: Vec<u64> = sample_ids
        .iter()
        .map(|&raw| heeranjid::HeerIdDesc::from_i64(raw).unwrap().timestamp_ms())
        .collect();
    assert!(
        sample_ts.windows(2).all(|w| w[0] >= w[1]),
        "ORDER BY id returns reverse-chronological rows: {sample_ts:?}"
    );

    // (e) Column layout — old columns gone, only `id` and `parent_id` remain.
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'nodes_self_fk' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(cols, vec!["id".to_string(), "parent_id".to_string()]);

    // --- Cleanup ---
    let _ = drop_autofill_trigger_for_table(&pg_client, "nodes_self_fk").await;
    pool.execute("DROP TABLE IF EXISTS nodes_self_fk CASCADE")
        .await
        .unwrap();
}
