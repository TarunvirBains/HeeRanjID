//! Integration test: partitioned-parent asc -> desc migration (spec §7.7).
//!
//! Exercises the canonical PG 13+ partitioned-table migration workflow
//! end-to-end against a live Postgres: preparation (parent-level column +
//! trigger that propagates to leaves), per-leaf backfill, per-partition
//! concurrent UNIQUE index build + parent attach, NOT NULL fast path, and
//! atomic cutover using `ADD PRIMARY KEY (cols)` rather than `USING INDEX`
//! (which Postgres does not support on partitioned parents).
//!
//! # Fixture shape
//!
//! A range-partitioned parent keyed by `bucket` with the canonical
//! composite PK `(bucket, id)` (spec §7.7: every parent-level UNIQUE/PK
//! must include all partition-key columns). Two partitions split the
//! `bucket` space in half: `lo = [0, 500)` and `hi = [500, 1000)`.
//!
//! # Why dual-connect
//!
//! Same as the other migrate_* tests: `install_*` helpers run against a
//! `tokio_postgres::Client` (because that's what the helpers take), while
//! fixture + migration SQL runs through a `sqlx::PgPool`.
//!
//! # Version gate
//!
//! Automatic BEFORE-trigger propagation from partitioned parent to leaves
//! requires PG 13+. Older servers need per-leaf trigger attachment via
//! the §7.7 fallback path; this test focuses on the canonical PG 13+
//! shape and skips cleanly (no failure) on older servers.
//!
//! Task 14 of the v0.3.0 descending-sort IDs plan.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, install_all_desc_support, install_autofill_trigger_for_table,
    install_schema, seed_default_node,
};
use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;
use tokio_postgres::NoTls;

#[tokio::test(flavor = "multi_thread")]
async fn partitioned_parent_migration_end_to_end() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live partitioned migration test");
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

    // --- Version gate: canonical §7.7 shape needs PG 13+ ---
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&pool)
        .await
        .expect("SHOW server_version_num");
    let v: u32 = version.parse().unwrap();
    if v < 130000 {
        eprintln!("SKIP: partitioned-table migration test requires PG 13+ (got {version})");
        return;
    }

    // --- Base schema + desc support (idempotent) ---
    install_schema(&pg_client).await.expect("install_schema");
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");

    // `heerid_next()` reads the session `heer.node_id`; set on the
    // tokio-postgres client (used for trigger install DDL).
    pg_client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set_heer_node_id on tokio-postgres client");

    // --- Fixture: drop any leftover, recreate parent + two partitions ---
    pool.execute("DROP TABLE IF EXISTS part_events CASCADE")
        .await
        .expect("drop pre-existing parent");

    pool.execute(
        "CREATE TABLE part_events (
            bucket int NOT NULL,
            id bigint NOT NULL DEFAULT heerid_next(),
            payload text NOT NULL DEFAULT 'x',
            PRIMARY KEY (bucket, id)
        ) PARTITION BY RANGE (bucket)",
    )
    .await
    .expect("create partitioned parent");

    pool.execute(
        "CREATE TABLE part_events_lo PARTITION OF part_events FOR VALUES FROM (0) TO (500)",
    )
    .await
    .expect("create lo partition");
    pool.execute(
        "CREATE TABLE part_events_hi PARTITION OF part_events FOR VALUES FROM (500) TO (1000)",
    )
    .await
    .expect("create hi partition");

    // Pin a sqlx connection so `set_heer_node_id(1)` persists for the
    // subsequent INSERTs that invoke `heerid_next()` via DEFAULT.
    let mut fixture_conn = pool.acquire().await.expect("acquire fixture conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *fixture_conn)
        .await
        .expect("set_heer_node_id on fixture conn");

    // 50 rows into the lo partition (buckets 0..500), 50 into hi
    // (buckets 500..1000). Use distinct bucket values so per-partition
    // row counts are exactly 50/50.
    sqlx::query(
        "INSERT INTO part_events (bucket) \
         SELECT g FROM generate_series(0, 49) g",
    )
    .execute(&mut *fixture_conn)
    .await
    .expect("seed lo partition");
    sqlx::query(
        "INSERT INTO part_events (bucket) \
         SELECT 500 + g FROM generate_series(0, 49) g",
    )
    .execute(&mut *fixture_conn)
    .await
    .expect("seed hi partition");
    drop(fixture_conn);

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events")
        .fetch_one(&pool)
        .await
        .expect("parent count");
    let lo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events_lo")
        .fetch_one(&pool)
        .await
        .expect("lo count");
    let hi_count: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events_hi")
        .fetch_one(&pool)
        .await
        .expect("hi count");
    assert_eq!(total, 100, "fixture should have 100 rows total");
    assert_eq!(lo_count, 50, "lo partition should have 50 rows");
    assert_eq!(hi_count, 50, "hi partition should have 50 rows");

    // --- Phase 1: preparation — add sibling column + parent trigger ---
    // ADD COLUMN on the parent propagates to all partitions automatically.
    pool.execute("ALTER TABLE part_events ADD COLUMN id_desc bigint")
        .await
        .expect("add id_desc on parent");

    // PG 13+ routes the BEFORE trigger attached to the parent through to
    // every leaf partition — one install call covers all partitions.
    install_autofill_trigger_for_table(
        &pg_client,
        "part_events",
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install_autofill_trigger_for_table on parent");

    // --- Phase 2: backfill PER LEAF PARTITION (spec §7.7 step 3) ---
    // The procedure needs each leaf's physical name; parent-level calls
    // won't work because the procedure operates on a concrete relation.
    pool.execute("CALL heeranjid_bulk_backfill('part_events_lo','id','id_desc','heer',50)")
        .await
        .expect("backfill lo");
    pool.execute("CALL heeranjid_bulk_backfill('part_events_hi','id','id_desc','heer',50)")
        .await
        .expect("backfill hi");

    // --- Phase 3: verify no NULLs remain (aggregates across partitions) ---
    let missing: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events WHERE id_desc IS NULL")
        .fetch_one(&pool)
        .await
        .expect("count NULL id_desc");
    assert_eq!(missing, 0, "no NULL id_desc after per-partition backfill");

    // Extra assurance: backfill correctness across partitions.
    let wrong: i64 =
        sqlx::query_scalar("SELECT count(*) FROM part_events WHERE id_desc <> heerid_to_desc(id)")
            .fetch_one(&pool)
            .await
            .expect("count divergent rows");
    assert_eq!(wrong, 0, "id_desc must equal heerid_to_desc(id) everywhere");

    // --- Phase 4: per-partition UNIQUE index builds + parent ATTACH (§7.7 step 5) ---
    // Parent-level UNIQUE placeholder created ON ONLY (leaves unindexed);
    // must be UNIQUE for `ATTACH PARTITION` to absorb unique child indexes.
    pool.execute(
        "CREATE UNIQUE INDEX part_events_bucket_id_desc_idx \
             ON ONLY part_events (bucket, id_desc)",
    )
    .await
    .expect("create parent UNIQUE placeholder");

    // Per-partition concurrent, non-blocking unique builds.
    pool.execute(
        "CREATE UNIQUE INDEX CONCURRENTLY part_events_lo_bucket_id_desc_idx \
             ON part_events_lo (bucket, id_desc)",
    )
    .await
    .expect("concurrent unique on lo");
    pool.execute(
        "CREATE UNIQUE INDEX CONCURRENTLY part_events_hi_bucket_id_desc_idx \
             ON part_events_hi (bucket, id_desc)",
    )
    .await
    .expect("concurrent unique on hi");

    // Catalog-only attach; fast. Parent placeholder becomes valid once
    // every partition is attached.
    pool.execute(
        "ALTER INDEX part_events_bucket_id_desc_idx \
             ATTACH PARTITION part_events_lo_bucket_id_desc_idx",
    )
    .await
    .expect("attach lo index");
    pool.execute(
        "ALTER INDEX part_events_bucket_id_desc_idx \
             ATTACH PARTITION part_events_hi_bucket_id_desc_idx",
    )
    .await
    .expect("attach hi index");

    // --- Phase 5: NOT NULL fast-path at the parent level ---
    pool.execute(
        "ALTER TABLE part_events \
             ADD CONSTRAINT part_events_id_desc_nn CHECK (id_desc IS NOT NULL) NOT VALID",
    )
    .await
    .expect("add NOT VALID check");
    pool.execute("ALTER TABLE part_events VALIDATE CONSTRAINT part_events_id_desc_nn")
        .await
        .expect("validate check");
    pool.execute("ALTER TABLE part_events ALTER COLUMN id_desc SET NOT NULL")
        .await
        .expect("set NOT NULL");
    pool.execute("ALTER TABLE part_events DROP CONSTRAINT part_events_id_desc_nn")
        .await
        .expect("drop redundant check");

    // --- Phase 6: atomic cutover (§7.7 step 7) ---
    //
    // Note: `ADD PRIMARY KEY (cols)` rather than `USING INDEX` — the
    // latter is unsupported on partitioned parents. Postgres may scan
    // partitions and/or build replacement indexes; with 100 rows this
    // should still complete in well under a second, but we measure it
    // for the report.
    let cutover_start = std::time::Instant::now();
    let mut tx = pool.begin().await.expect("begin cutover tx");

    sqlx::query("ALTER TABLE part_events DROP CONSTRAINT part_events_pkey")
        .execute(&mut *tx)
        .await
        .expect("drop old pkey");
    sqlx::query("ALTER TABLE part_events ADD PRIMARY KEY (bucket, id_desc)")
        .execute(&mut *tx)
        .await
        .expect("add new pkey on (bucket, id_desc)");
    sqlx::query("ALTER TABLE part_events ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await
        .expect("set desc DEFAULT on id_desc");
    sqlx::query("ALTER TABLE part_events ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await
        .expect("drop DEFAULT on id");
    sqlx::query("ALTER TABLE part_events DROP COLUMN id")
        .execute(&mut *tx)
        .await
        .expect("drop old id column");
    sqlx::query("DROP TRIGGER zzz_part_events_autofill_desc ON part_events")
        .execute(&mut *tx)
        .await
        .expect("drop autofill trigger");
    sqlx::query("DROP FUNCTION zzz_part_events_autofill_desc() CASCADE")
        .execute(&mut *tx)
        .await
        .expect("drop autofill trigger fn");
    sqlx::query("ALTER TABLE part_events RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await
        .expect("rename id_desc -> id");

    tx.commit().await.expect("commit cutover");
    let cutover_elapsed = cutover_start.elapsed();
    eprintln!(
        "partitioned cutover (BEGIN..COMMIT incl. ADD PRIMARY KEY) took {:?}",
        cutover_elapsed
    );

    // --- Post-cutover assertions ---

    // (a) Total row count unchanged.
    let post_total: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events")
        .fetch_one(&pool)
        .await
        .expect("post total count");
    assert_eq!(post_total, 100, "total row count preserved");

    // (b) Per-partition counts unchanged.
    let post_lo: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events_lo")
        .fetch_one(&pool)
        .await
        .expect("post lo count");
    let post_hi: i64 = sqlx::query_scalar("SELECT count(*) FROM part_events_hi")
        .fetch_one(&pool)
        .await
        .expect("post hi count");
    assert_eq!(post_lo, 50, "lo partition count preserved");
    assert_eq!(post_hi, 50, "hi partition count preserved");

    // (c) ORDER BY id returns reverse-chronological logical timestamps.
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM part_events ORDER BY id LIMIT 3")
        .fetch_all(&pool)
        .await
        .expect("fetch sorted ids");
    assert_eq!(ids.len(), 3, "expected 3 rows sampled");
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

    // (d) Column layout at the parent: exactly `bucket`, `id`, `payload`
    //     (the old `id` column is gone; `id_desc` has been renamed to `id`).
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_name = 'part_events' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .expect("list parent columns");
    assert_eq!(
        cols,
        vec![
            "bucket".to_string(),
            "id".to_string(),
            "payload".to_string()
        ],
        "parent columns after cutover"
    );

    // (e) New PK is (bucket, id) — query pg_constraint to confirm the
    //     column order + composition matches.
    let pk_cols: Vec<String> = sqlx::query_scalar(
        "SELECT a.attname::text
         FROM pg_constraint c
         JOIN pg_class t ON t.oid = c.conrelid
         JOIN unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord) ON true
         JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
         WHERE t.relname = 'part_events' AND c.contype = 'p'
         ORDER BY k.ord",
    )
    .fetch_all(&pool)
    .await
    .expect("fetch PK columns");
    assert_eq!(
        pk_cols,
        vec!["bucket".to_string(), "id".to_string()],
        "new PK must be (bucket, id) — id_desc renamed into id"
    );

    // --- Cleanup ---
    pool.execute("DROP TABLE IF EXISTS part_events CASCADE")
        .await
        .expect("final cleanup drop");
}
