//! Runnable example: parent + child FK-cascade asc -> desc migration.
//!
//! Mirrors the §7.2 playbook of the descending-sort IDs plan against a
//! live Postgres: a parent (`ex_parents_fk`) and child (`ex_children_fk`)
//! pair migrate their `id` / `parent_id` columns to descending-sort
//! siblings, demonstrating the one-transaction cutover that drops both
//! FK + both PK, promotes the new parent index, adds the new child FK
//! `NOT VALID`, then validates after `COMMIT`.
//!
//! # Running
//!
//! ```text
//! DATABASE_URL=postgres://... cargo run -p heeranjid --features sqlx \
//!     --features postgres --example migrate_asc_to_desc_with_fk
//! ```
//!
//! Smaller fixture than the §7.1 example — 10k parents + 10k children —
//! so the full flow completes in seconds on commodity hardware. If
//! `DATABASE_URL` is unset, prints a SKIP banner and exits 0.
//!
//! # Connection contexts
//!
//! Three distinct connection/transaction contexts, same rationale as the
//! single-table example:
//!
//! * **Bare `&PgPool`** — preparation DDL and `CREATE INDEX
//!   CONCURRENTLY`. `CONCURRENTLY` cannot run inside a transaction
//!   block; a bare pool connection is the correct context.
//! * **Top-level `CALL heeranjid_bulk_backfill(...)`** — the procedure
//!   issues `COMMIT` inside its loop and must therefore run at the top
//!   level, never inside an application-opened transaction (spec §5.1).
//! * **Single `pool.begin().await?`** — the cutover. All eight catalog
//!   changes (drop FK, drop both PKs, promote parent index, add child
//!   NOT VALID FK, drop old columns, drop triggers, rename columns)
//!   commit atomically. The subsequent `VALIDATE CONSTRAINT` runs
//!   *outside* that transaction to avoid holding the long parent scan
//!   lock across the cutover commit.
//!
//! # Dual-connect
//!
//! The install helpers (`install_schema`, `install_all_desc_support`,
//! `install_autofill_trigger_for_table`) take a
//! `tokio_postgres::GenericClient`; the migration runs through an
//! `sqlx::PgPool`. We open both against the same `DATABASE_URL`.
//!
//! # Memory characteristics (§8.1)
//!
//! * Rust-side memory: O(1) — no row data enters Rust, only DDL and
//!   one `CALL` per table.
//! * Postgres-side memory per commit: O(batch_size) (2000 here).
//! * Total Rust memory during the migration: ~the sqlx connection pool.
//!
//! Task 15 of the v0.3.0 descending-sort IDs plan.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, install_all_desc_support, install_autofill_trigger_for_table,
    install_schema, seed_default_node,
};
use sqlx::postgres::PgPoolOptions;
use tokio_postgres::NoTls;

/// Parent table used by this example. Unique names so repeated runs
/// don't collide; `DROP TABLE IF EXISTS ... CASCADE` at start + end
/// keeps it idempotent.
const PARENT: &str = "ex_parents_fk";
/// Child table used by this example.
const CHILD: &str = "ex_children_fk";
/// Number of rows per table in the fixture.
const ROWS: i64 = 10_000;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set; nothing to migrate against");
        return Ok(());
    };

    // --- Dual-connect: tokio-postgres for install helpers, sqlx for the rest ---
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;

    let (pg_client, pg_conn) = tokio_postgres::connect(&url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = pg_conn.await {
            eprintln!("postgres connection error: {e}");
        }
    });

    eprintln!("Phase 0: installing base schema + desc support (idempotent)");
    install_schema(&pg_client).await?;
    seed_default_node(&pg_client).await?;
    install_all_desc_support(&pg_client).await?;

    // `heerid_next()` reads the session's `heer.node_id`. Set it on both
    // the tokio-postgres client (used for trigger install DDL) and on
    // the sqlx connection that seeds rows, so `DEFAULT heerid_next()`
    // does not fail during the fixture INSERT.
    pg_client.execute("SELECT set_heer_node_id(1)", &[]).await?;

    // --- Fixture ---
    eprintln!("Phase 0: building fixture ({ROWS} parents + {ROWS} children)");
    sqlx::query(&format!("DROP TABLE IF EXISTS {CHILD} CASCADE"))
        .execute(&pool)
        .await?;
    sqlx::query(&format!("DROP TABLE IF EXISTS {PARENT} CASCADE"))
        .execute(&pool)
        .await?;
    sqlx::query(&format!(
        "CREATE TABLE {PARENT} (id bigint PRIMARY KEY DEFAULT heerid_next())"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "CREATE TABLE {CHILD} ( \
             id bigint PRIMARY KEY DEFAULT heerid_next(), \
             parent_id bigint NOT NULL REFERENCES {PARENT}(id) \
         )"
    ))
    .execute(&pool)
    .await?;

    // Seed on one pinned connection so `set_heer_node_id(1)` persists
    // across the INSERTs.
    let mut seed_conn = pool.acquire().await?;
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *seed_conn)
        .await?;
    sqlx::query(&format!(
        "INSERT INTO {PARENT} SELECT heerid_next() FROM generate_series(1, {ROWS})"
    ))
    .execute(&mut *seed_conn)
    .await?;
    // Each child picks a random parent; INSERT avoids a cross-join
    // blowup by using `generate_series` on both sides and a correlated
    // `ORDER BY random()` sample for the parent id.
    sqlx::query(&format!(
        "INSERT INTO {CHILD} (parent_id) \
         SELECT (SELECT id FROM {PARENT} ORDER BY random() LIMIT 1) \
         FROM generate_series(1, {ROWS})"
    ))
    .execute(&mut *seed_conn)
    .await?;
    drop(seed_conn);

    let parent_count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {PARENT}"))
        .fetch_one(&pool)
        .await?;
    let child_count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {CHILD}"))
        .fetch_one(&pool)
        .await?;
    assert_eq!(parent_count, ROWS, "parent fixture should have {ROWS} rows");
    assert_eq!(child_count, ROWS, "child fixture should have {ROWS} rows");

    // --- Phase 1: preparation ---
    eprintln!("Phase 1: preparation (add sibling columns + per-table triggers)");
    sqlx::query(&format!("ALTER TABLE {PARENT} ADD COLUMN id_desc bigint"))
        .execute(&pool)
        .await?;
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} ADD COLUMN parent_id_desc bigint"
    ))
    .execute(&pool)
    .await?;

    install_autofill_trigger_for_table(
        &pg_client,
        PARENT,
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await?;
    install_autofill_trigger_for_table(
        &pg_client,
        CHILD,
        &[ColumnPair {
            src: "parent_id",
            dst: "parent_id_desc",
        }],
        IdKind::Heer,
    )
    .await?;

    // --- Phase 2: backfill (top-level CALL, not inside a transaction) ---
    eprintln!("Phase 2: backfill (parent then child, top-level CALL each)");
    sqlx::query(&format!(
        "CALL heeranjid_bulk_backfill('{PARENT}','id','id_desc','heer',2000)"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "CALL heeranjid_bulk_backfill('{CHILD}','parent_id','parent_id_desc','heer',2000)"
    ))
    .execute(&pool)
    .await?;

    let missing_parent: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {PARENT} WHERE id_desc IS NULL"
    ))
    .fetch_one(&pool)
    .await?;
    let missing_child: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {CHILD} WHERE parent_id_desc IS NULL"
    ))
    .fetch_one(&pool)
    .await?;
    assert_eq!(missing_parent, 0, "no NULL id_desc after parent backfill");
    assert_eq!(
        missing_child, 0,
        "no NULL parent_id_desc after child backfill"
    );

    // --- Phase 3: index build (outside txn) + NOT NULL fast path ---
    eprintln!("Phase 3: concurrent index build + NOT NULL fast path");
    let parent_idx = format!("idx_{PARENT}_id_desc");
    let parent_check = format!("{PARENT}_id_desc_nn");
    let child_check = format!("{CHILD}_parent_id_desc_nn");

    sqlx::query(&format!(
        "CREATE UNIQUE INDEX CONCURRENTLY {parent_idx} ON {PARENT} (id_desc)"
    ))
    .execute(&pool)
    .await?;

    // Parent: NOT NULL fast path.
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} ADD CONSTRAINT {parent_check} CHECK (id_desc IS NOT NULL) NOT VALID"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} VALIDATE CONSTRAINT {parent_check}"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} ALTER COLUMN id_desc SET NOT NULL"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} DROP CONSTRAINT {parent_check}"
    ))
    .execute(&pool)
    .await?;

    // Child: NOT NULL fast path for parent_id_desc (FK-bearing column).
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} ADD CONSTRAINT {child_check} \
         CHECK (parent_id_desc IS NOT NULL) NOT VALID"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} VALIDATE CONSTRAINT {child_check}"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} ALTER COLUMN parent_id_desc SET NOT NULL"
    ))
    .execute(&pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} DROP CONSTRAINT {child_check}"
    ))
    .execute(&pool)
    .await?;

    // --- Phase 4: atomic cutover ---
    //
    // Order per plan Task 13 / §7.2:
    //   1. Drop old child FK.
    //   2. Drop old parent PK.
    //   3. Promote parent's new unique index to the new PK.
    //   4. Set DEFAULT heerid_next_desc() on parent.id_desc; drop DEFAULT on id.
    //   5. Add the new child FK `NOT VALID` (validates lazily after commit).
    //   6. Drop old `id` / `parent_id` columns.
    //   7. Drop the two autofill triggers + underlying functions.
    //   8. Rename `id_desc -> id` and `parent_id_desc -> parent_id`.
    eprintln!("Phase 4: atomic cutover (single transaction)");
    let new_child_fk = format!("{CHILD}_parent_id_desc_fkey");
    let mut tx = pool.begin().await?;

    // 1. Drop the old child FK.
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} DROP CONSTRAINT {CHILD}_parent_id_fkey"
    ))
    .execute(&mut *tx)
    .await?;

    // 2. Drop the old parent PK.
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} DROP CONSTRAINT {PARENT}_pkey"
    ))
    .execute(&mut *tx)
    .await?;

    // 3. Promote the parent's new unique index to be the new PK.
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} ADD CONSTRAINT {PARENT}_pkey PRIMARY KEY USING INDEX {parent_idx}"
    ))
    .execute(&mut *tx)
    .await?;

    // 4. DEFAULTs: desc generator on id_desc; strip the asc DEFAULT.
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {PARENT} ALTER COLUMN id DROP DEFAULT"
    ))
    .execute(&mut *tx)
    .await?;

    // 5. Add the new child FK referencing the parent's new PK, NOT VALID.
    //    Per §7.2 and Task 13, we reference the parent's freshly-promoted
    //    PK column — which is still named `id_desc` at this point within
    //    the cutover tx — but `USING INDEX` on step 3 makes that column
    //    the unique target. Because we rename `id_desc -> id` below
    //    (step 8), the FK's referenced column name is resolved at catalog
    //    time and survives the rename.
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} ADD CONSTRAINT {new_child_fk} \
         FOREIGN KEY (parent_id_desc) REFERENCES {PARENT}(id_desc) NOT VALID"
    ))
    .execute(&mut *tx)
    .await?;

    // 6. Drop old `id` / `parent_id` columns.
    sqlx::query(&format!("ALTER TABLE {CHILD} DROP COLUMN parent_id"))
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!("ALTER TABLE {PARENT} DROP COLUMN id"))
        .execute(&mut *tx)
        .await?;

    // 7. Drop autofill triggers + functions on both tables.
    sqlx::query(&format!(
        "DROP TRIGGER zzz_{PARENT}_autofill_desc ON {PARENT}"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!(
        "DROP FUNCTION zzz_{PARENT}_autofill_desc() CASCADE"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!(
        "DROP TRIGGER zzz_{CHILD}_autofill_desc ON {CHILD}"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!(
        "DROP FUNCTION zzz_{CHILD}_autofill_desc() CASCADE"
    ))
    .execute(&mut *tx)
    .await?;

    // 8. Rename desc columns into the canonical names.
    sqlx::query(&format!("ALTER TABLE {PARENT} RENAME COLUMN id_desc TO id"))
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} RENAME COLUMN parent_id_desc TO parent_id"
    ))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // --- Phase 5: validate the new child FK (OUTSIDE the cutover tx) ---
    //
    // Per §7.2: running VALIDATE outside the cutover transaction keeps
    // the cutover itself at catalog-update latency; the validation scan
    // takes a weaker lock and can run concurrently with writes.
    eprintln!("Phase 5: VALIDATE CONSTRAINT (outside cutover tx)");
    sqlx::query(&format!(
        "ALTER TABLE {CHILD} VALIDATE CONSTRAINT {new_child_fk}"
    ))
    .execute(&pool)
    .await?;

    // --- Post-cutover assertions ---
    eprintln!("Phase 6: post-cutover assertions");

    // (a) FK integrity: every child row's parent_id resolves to a parent.
    let orphans: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {CHILD} c \
         LEFT JOIN {PARENT} p ON c.parent_id = p.id \
         WHERE p.id IS NULL"
    ))
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        orphans, 0,
        "FK integrity: every child's parent_id must resolve"
    );

    // (b) Parent rows come back in reverse-chronological order via
    //     ORDER BY id ASC, because `id` now stores the desc flip.
    let parent_ids: Vec<i64> =
        sqlx::query_scalar(&format!("SELECT id FROM {PARENT} ORDER BY id LIMIT 100"))
            .fetch_all(&pool)
            .await?;
    assert_eq!(parent_ids.len(), 100, "expected 100 parent rows sampled");
    let logical: Vec<u64> = parent_ids
        .iter()
        .map(|&raw| {
            heeranjid::HeerIdDesc::from_i64(raw)
                .expect("stored value is a valid HeerIdDesc")
                .timestamp_ms()
        })
        .collect();
    assert!(
        logical.windows(2).all(|w| w[0] >= w[1]),
        "parent ORDER BY id yields reverse-chronological rows: {logical:?}"
    );

    // (c) Old columns are gone on both tables.
    let parent_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_name = $1 ORDER BY column_name",
    )
    .bind(PARENT)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        parent_cols,
        vec!["id".to_string()],
        "parent should have only `id` post-cutover"
    );
    let child_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_name = $1 ORDER BY column_name",
    )
    .bind(CHILD)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        child_cols,
        vec!["id".to_string(), "parent_id".to_string()],
        "child should have only `id` + `parent_id` post-cutover"
    );

    // --- Cleanup ---
    eprintln!("Phase 7: cleanup (dropping fixture tables)");
    sqlx::query(&format!("DROP TABLE IF EXISTS {CHILD} CASCADE"))
        .execute(&pool)
        .await?;
    sqlx::query(&format!("DROP TABLE IF EXISTS {PARENT} CASCADE"))
        .execute(&pool)
        .await?;

    eprintln!("migration complete");
    Ok(())
}
