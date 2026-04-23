//! Parent + child FK cascade asc→desc migration (Task 13 / spec §7.2).
//!
//! Exercises the parent-plus-child playbook end-to-end against a live
//! Postgres: seed a non-nullable-PK parent table and a child table whose
//! FK points at the parent's PK, then migrate both tables to descending
//! siblings under a single atomic cutover transaction.
//!
//! # Why dual-connect
//!
//! `heeranjid::postgres_schema::install_*` helpers take a
//! `tokio_postgres::GenericClient`, but the bulk SQL / fixtures run
//! through a `sqlx::PgPool`. We hold both connections side by side — a
//! pattern called out in the plan — rather than pushing a dual-executor
//! abstraction into the library. This matches the plan §7.2 runbook:
//! trigger install is a one-shot schema step, whereas the pool drives
//! the streaming backfill + cutover.
//!
//! # Cutover ordering (critical)
//!
//! Per §7.2, the single transaction drops the child FK first, then the
//! parent PK, promotes `idx_parents_id_desc` into the new PK, adds the
//! new child FK as `NOT VALID` referencing the *existing* (pre-rename)
//! `parents.id_desc`, then drops old columns/triggers, and finally
//! renames. `VALIDATE CONSTRAINT` runs after COMMIT so it doesn't block
//! the cutover window.
//!
//! Task 13 of the v0.3.0 descending-sort IDs plan.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, drop_autofill_trigger_for_table, install_all_desc_support,
    install_autofill_trigger_for_table, install_schema, seed_default_node,
};
use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;

/// Spawn a dedicated `tokio_postgres::Client` alongside the sqlx pool.
///
/// Returns `None` if `DATABASE_URL` isn't set so the test can skip (same
/// contract as the rest of the `heeranjid-sqlx` integration tests).
async fn dual_connect(url: &str) -> tokio_postgres::Client {
    let (client, conn) = tokio_postgres::connect(url, tokio_postgres::NoTls)
        .await
        .expect("tokio-postgres connect");
    // Drive the connection in the background. For a test we don't join
    // on completion; the tokio runtime will tear it down at test exit.
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("tokio-postgres connection error: {e}");
        }
    });
    client
}

#[tokio::test(flavor = "multi_thread")]
async fn parent_child_fk_migration_end_to_end() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
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
    // seed_default_node is safe to skip if already seeded; swallow the
    // unique-violation by ignoring errors from a fresh attempt.
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");

    // --- Fixture: drop any leftover from a previous failed run, then recreate ---
    pool.execute("DROP TABLE IF EXISTS children CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS parents CASCADE")
        .await
        .unwrap();

    pool.execute("CREATE TABLE parents (id bigint PRIMARY KEY DEFAULT heerid_next())")
        .await
        .unwrap();
    pool.execute(
        "CREATE TABLE children (
            id bigint PRIMARY KEY DEFAULT heerid_next(),
            parent_id bigint NOT NULL REFERENCES parents(id)
         )",
    )
    .await
    .unwrap();

    // Pin a sqlx connection for the fixture inserts so that the
    // `set_heer_node_id(1)` session setting persists across the
    // `heerid_next()` calls in the INSERT statements — without pinning,
    // the pool may hand a fresh connection to each query and
    // `current_heer_node_id()` inside `heerid_next()` would raise.
    let mut fixture_conn = pool.acquire().await.expect("acquire fixture conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *fixture_conn)
        .await
        .expect("set_heer_node_id on fixture conn");

    // 100 parent rows.
    sqlx::query("INSERT INTO parents SELECT heerid_next() FROM generate_series(1, 100)")
        .execute(&mut *fixture_conn)
        .await
        .unwrap();

    // 10 children per parent = 1000 children total. Pick parent IDs
    // deterministically from the seeded parents.
    sqlx::query(
        "INSERT INTO children (id, parent_id)
         SELECT heerid_next(), p.id
         FROM parents p, generate_series(1, 10) g",
    )
    .execute(&mut *fixture_conn)
    .await
    .unwrap();
    drop(fixture_conn);

    let parent_count: i64 = sqlx::query_scalar("SELECT count(*) FROM parents")
        .fetch_one(&pool)
        .await
        .unwrap();
    let child_count: i64 = sqlx::query_scalar("SELECT count(*) FROM children")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(parent_count, 100);
    assert_eq!(child_count, 1000);

    // --- Phase 1: preparation — add desc columns + install triggers ---
    pool.execute("ALTER TABLE parents ADD COLUMN id_desc bigint")
        .await
        .unwrap();
    pool.execute("ALTER TABLE children ADD COLUMN parent_id_desc bigint")
        .await
        .unwrap();

    // Parents: PK flip (id -> id_desc).
    install_autofill_trigger_for_table(
        &pg_client,
        "parents",
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install trigger on parents");

    // Children: FK flip (parent_id -> parent_id_desc). PK-only approach on
    // parents + FK-only on children — keeps the child's own `id` column
    // untouched so this test focuses specifically on the FK cascade.
    install_autofill_trigger_for_table(
        &pg_client,
        "children",
        &[ColumnPair {
            src: "parent_id",
            dst: "parent_id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install trigger on children");

    // --- Phase 2: backfill (each table, top-level CALL, no outer tx) ---
    pool.execute("CALL heeranjid_bulk_backfill('parents','id','id_desc','heer',2000)")
        .await
        .unwrap();
    pool.execute(
        "CALL heeranjid_bulk_backfill('children','parent_id','parent_id_desc','heer',2000)",
    )
    .await
    .unwrap();

    let missing_parents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM parents WHERE id_desc IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    let missing_children: i64 =
        sqlx::query_scalar("SELECT count(*) FROM children WHERE parent_id_desc IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(missing_parents, 0, "all parents backfilled");
    assert_eq!(missing_children, 0, "all children backfilled");

    // --- Phase 3: indexes + NOT NULL fast path (outside a transaction) ---
    pool.execute("CREATE UNIQUE INDEX CONCURRENTLY idx_parents_id_desc ON parents (id_desc)")
        .await
        .unwrap();
    pool.execute(
        "CREATE INDEX CONCURRENTLY idx_children_parent_id_desc ON children (parent_id_desc)",
    )
    .await
    .unwrap();

    for (tbl, col, cname) in [
        ("parents", "id_desc", "parents_id_desc_nn"),
        ("children", "parent_id_desc", "children_parent_id_desc_nn"),
    ] {
        pool.execute(
            format!("ALTER TABLE {tbl} ADD CONSTRAINT {cname} CHECK ({col} IS NOT NULL) NOT VALID")
                .as_str(),
        )
        .await
        .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} VALIDATE CONSTRAINT {cname}").as_str())
            .await
            .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} ALTER COLUMN {col} SET NOT NULL").as_str())
            .await
            .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} DROP CONSTRAINT {cname}").as_str())
            .await
            .unwrap();
    }

    // --- Phase 4: atomic cutover — ONE transaction covering BOTH tables ---
    //
    // Ordering (§7.2):
    //   1. Drop child FK (so parent PK is free to move).
    //   2. Drop parent PK.
    //   3. Promote `idx_parents_id_desc` as the new parent PK.
    //   4. Add new child FK (NOT VALID) referencing `parents(id_desc)` —
    //      the column is still named `id_desc` at this point; rename
    //      happens at the end of the tx.
    //   5. Swap defaults / drop old columns / drop triggers on both tables.
    //   6. Rename new columns into place on both tables.
    let mut tx = pool.begin().await.unwrap();

    // (1) Drop child FK. The default auto-generated name for
    //     `parent_id bigint NOT NULL REFERENCES parents(id)` is
    //     `children_parent_id_fkey`.
    sqlx::query("ALTER TABLE children DROP CONSTRAINT children_parent_id_fkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (2) Drop parent PK.
    sqlx::query("ALTER TABLE parents DROP CONSTRAINT parents_pkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (3) Promote desc index to PK.
    sqlx::query("ALTER TABLE parents ADD CONSTRAINT parents_pkey PRIMARY KEY USING INDEX idx_parents_id_desc")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (4) Add new child FK as NOT VALID, referencing the still-named
    //     `id_desc` column on parents.
    sqlx::query(
        "ALTER TABLE children
            ADD CONSTRAINT children_parent_id_desc_fkey
            FOREIGN KEY (parent_id_desc) REFERENCES parents(id_desc) NOT VALID",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (5) Swap defaults + drop old columns + drop triggers.
    sqlx::query("ALTER TABLE parents ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE parents ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE parents DROP COLUMN id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE children DROP COLUMN parent_id")
        .execute(&mut *tx)
        .await
        .unwrap();

    // Drop both per-table trigger functions (+ triggers via CASCADE).
    sqlx::query("DROP TRIGGER IF EXISTS zzz_parents_autofill_desc ON parents")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS zzz_parents_autofill_desc() CASCADE")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS zzz_children_autofill_desc ON children")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS zzz_children_autofill_desc() CASCADE")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (6) Rename new columns into place.
    sqlx::query("ALTER TABLE parents RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE children RENAME COLUMN parent_id_desc TO parent_id")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // --- Phase 5: validate the deferred FK outside the cutover tx ---
    pool.execute("ALTER TABLE children VALIDATE CONSTRAINT children_parent_id_desc_fkey")
        .await
        .unwrap();

    // --- Assertions ---

    // (a) FK integrity: every child still points at a real parent.
    let orphans: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM children c
         LEFT JOIN parents p ON c.parent_id = p.id
         WHERE p.id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        orphans, 0,
        "no orphaned children after FK cascade migration"
    );

    // (b) Parent row count preserved.
    let post_parent_count: i64 = sqlx::query_scalar("SELECT count(*) FROM parents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(post_parent_count, 100);

    // (c) Child row count preserved.
    let post_child_count: i64 = sqlx::query_scalar("SELECT count(*) FROM children")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(post_child_count, 1000);

    // (d) Column layout — old columns gone on both tables.
    let parent_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'parents' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(parent_cols, vec!["id".to_string()]);

    let child_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'children' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(child_cols, vec!["id".to_string(), "parent_id".to_string()]);

    // (e) Descending sort: plain ORDER BY id on parents yields newest-first.
    let parent_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM parents ORDER BY id LIMIT 5")
        .fetch_all(&pool)
        .await
        .unwrap();
    let parent_ts: Vec<u64> = parent_ids
        .iter()
        .map(|&raw| heeranjid::HeerIdDesc::from_i64(raw).unwrap().timestamp_ms())
        .collect();
    assert!(
        parent_ts.windows(2).all(|w| w[0] >= w[1]),
        "parents ORDER BY id returns reverse-chronological rows: {parent_ts:?}"
    );

    // --- Cleanup ---
    // Drop in FK order, then tidy up trigger helpers in case anything
    // survived a panic path.
    let _ = drop_autofill_trigger_for_table(&pg_client, "parents").await;
    let _ = drop_autofill_trigger_for_table(&pg_client, "children").await;
    pool.execute("DROP TABLE IF EXISTS children CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS parents CASCADE")
        .await
        .unwrap();
}
