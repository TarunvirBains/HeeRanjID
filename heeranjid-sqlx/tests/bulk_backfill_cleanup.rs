//! Integration test for the two-loop structure of
//! `heeranjid_bulk_backfill` (spec §5.1, §7.9).
//!
//! The procedure has a **fast loop** using `FOR UPDATE SKIP LOCKED` and
//! a **cleanup loop** using plain `FOR UPDATE`. The cleanup loop exists
//! because `SKIP LOCKED` can hide a row indefinitely when some other
//! transaction holds a lock every time the fast loop scans it. Without
//! the cleanup pass, the procedure would declare success while leaving
//! a subset of rows unflipped.
//!
//! This test exercises that pathway:
//!
//! 1. Seed a table with `id_desc` NULL on every row.
//! 2. Install the autofill trigger (so new writes populate `id_desc`,
//!    but the seeded rows remain NULL until backfill runs).
//! 3. Spawn a "locker" task that opens a transaction, `SELECT ... FOR
//!    UPDATE`s one specific row, sleeps briefly, then commits. This is
//!    the row the fast loop must skip and the cleanup loop must drain.
//! 4. While the locker holds its row, call `heeranjid_bulk_backfill`.
//!    The fast loop processes the unlocked rows, then the cleanup loop
//!    blocks on the locked row. When the locker commits, cleanup
//!    resumes and finishes.
//! 5. After the procedure returns, assert every row has `id_desc`
//!    populated — including the previously-locked row. That's the
//!    invariant the two-loop structure buys us.
//!
//! A 15-second overall timeout guards against regressions that would
//! cause the procedure to hang forever (e.g. the cleanup loop missing
//! its `SET LOCAL lock_timeout` reissue after `COMMIT`, which was the
//! round-5 review fix).

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, install_all_desc_support, install_autofill_trigger_for_table,
    install_schema, seed_default_node,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Acquire, Executor, Row};
use std::time::Duration;
use tokio_postgres::NoTls;

const TABLE: &str = "bulk_backfill_cleanup";
const SEED_ROWS: i64 = 20;
/// How long the locker holds its row before committing.
const LOCK_HOLD: Duration = Duration::from_millis(1500);
/// Overall timeout — if the procedure hangs past this, the cleanup loop
/// is broken.
const OVERALL_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_loop_drains_row_that_fast_loop_skipped() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };

    // --- Dual-connect ---
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("sqlx connect");
    let (pg_client, pg_conn) = tokio_postgres::connect(&url, NoTls)
        .await
        .expect("tokio-postgres connect");
    tokio::spawn(async move {
        let _ = pg_conn.await;
    });

    // --- Install schema + desc support + set node id ---
    install_schema(&pg_client).await.expect("install_schema");
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");
    pg_client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set_heer_node_id (tokio-postgres)");

    // --- Fixture: table with id_desc already present (nullable) so the
    //     seeded rows start with id_desc NULL and must be backfilled. ---
    pool.execute(format!("DROP TABLE IF EXISTS {TABLE} CASCADE").as_str())
        .await
        .expect("drop leftover");

    let mut seed_conn = pool.acquire().await.expect("acquire seed conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *seed_conn)
        .await
        .expect("set_heer_node_id on seed conn");
    sqlx::query(&format!(
        "CREATE TABLE {TABLE} (
             id bigint PRIMARY KEY DEFAULT heerid_next(),
             id_desc bigint
         )"
    ))
    .execute(&mut *seed_conn)
    .await
    .expect("create fixture");
    sqlx::query(&format!(
        "INSERT INTO {TABLE} (id) SELECT heerid_next() FROM generate_series(1, {SEED_ROWS})"
    ))
    .execute(&mut *seed_conn)
    .await
    .expect("seed rows");
    drop(seed_conn);

    // Install the trigger AFTER seeding — so the seeded rows keep
    // id_desc = NULL and become the backfill workload. New writes
    // (none in this test) would be trigger-populated.
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
    .expect("install trigger");

    // Sanity: every row starts with id_desc NULL.
    let null_count: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {TABLE} WHERE id_desc IS NULL"
    ))
    .fetch_one(&pool)
    .await
    .expect("null count");
    assert_eq!(null_count, SEED_ROWS, "every seeded row must start NULL");

    // Pick the row we're going to lock. We want a concrete id the
    // locker can target. Any row works; take the first one.
    let locked_id: i64 = sqlx::query_scalar(&format!("SELECT id FROM {TABLE} ORDER BY id LIMIT 1"))
        .fetch_one(&pool)
        .await
        .expect("pick victim row");

    // --- Run the test under an overall timeout so a hung cleanup loop
    //     fails fast instead of dangling the test runner. ---
    let run = async {
        // Spawn the locker: it takes its own pool connection, opens a
        // transaction, grabs the row lock, waits, then commits.
        let locker_pool = pool.clone();
        let locker = tokio::spawn(async move {
            let mut conn = locker_pool.acquire().await.expect("locker acquire conn");
            let mut tx = conn.begin().await.expect("locker begin");
            sqlx::query(&format!("SELECT id FROM {TABLE} WHERE id = $1 FOR UPDATE"))
                .bind(locked_id)
                .fetch_one(&mut *tx)
                .await
                .expect("locker FOR UPDATE");
            tokio::time::sleep(LOCK_HOLD).await;
            tx.commit().await.expect("locker commit");
        });

        // Give the locker a head-start so it's definitely holding the
        // lock before the backfill procedure starts scanning. A short
        // sleep is enough; the locker is holding the lock for 1.5s.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Call the backfill procedure on a separate pool connection.
        // CALL must run at the top level (the procedure commits per
        // batch internally), so we use `pool.execute` rather than
        // wrapping in our own transaction. Small batch size so the
        // fast loop iterates multiple times against the seed set.
        pool.execute(
            format!("CALL heeranjid_bulk_backfill('{TABLE}', 'id', 'id_desc', 'heer', 5)").as_str(),
        )
        .await
        .expect("bulk_backfill CALL");

        // The locker should have long since finished by this point; wait
        // on its handle for a clean shutdown and error propagation.
        locker.await.expect("locker task panicked");
    };

    tokio::time::timeout(OVERALL_TIMEOUT, run)
        .await
        .expect("backfill + cleanup loop completed within timeout (cleanup must not hang)");

    // --- Invariant: every row was populated, including the one the
    //     fast loop had to SKIP LOCKED past. ---
    let remaining_nulls: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {TABLE} WHERE id_desc IS NULL"
    ))
    .fetch_one(&pool)
    .await
    .expect("final null count");
    assert_eq!(
        remaining_nulls, 0,
        "cleanup loop must drain the row that was locked during the fast loop"
    );

    // And the formerly-locked row specifically carries the correct
    // flipped value — proving this wasn't populated by some other path.
    let row = sqlx::query(&format!(
        "SELECT id_desc, heerid_to_desc(id) AS expected FROM {TABLE} WHERE id = $1"
    ))
    .bind(locked_id)
    .fetch_one(&pool)
    .await
    .expect("fetch formerly-locked row");
    let actual: i64 = row.get::<i64, _>("id_desc");
    let expected: i64 = row.get::<i64, _>("expected");
    assert_eq!(
        actual, expected,
        "formerly-locked row must end with id_desc = heerid_to_desc(id)"
    );

    // Cleanup.
    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}
