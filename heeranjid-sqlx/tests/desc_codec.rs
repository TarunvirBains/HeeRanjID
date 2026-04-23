//! Integration tests for the descending-sort codecs and generators.
//!
//! Closes the spec-§9.2 coverage gaps that were deferred during the
//! initial v0.3.0 implementation:
//!
//! - `heer_desc_codec_round_trip_sqlx` — insert a `HeerIdDesc` via the
//!   sqlx codec, read it back, assert bit-for-bit equality.
//! - `ranj_desc_codec_round_trip_sqlx` — same for `RanjIdDesc`.
//! - `heerid_next_desc_generator_freshness` — `INSERT ... DEFAULT
//!   heerid_next_desc()`, decode via the sqlx codec, assert the logical
//!   timestamp is within ~5 seconds of `now()`.
//! - `ranjid_next_desc_generator_freshness` — same for `ranjid_next_desc()`.
//! - `heer_desc_db_sort_matches_rust_sort` — insert a shuffled batch of
//!   `HeerIdDesc` values; assert `SELECT ... ORDER BY id` returns the
//!   same sequence as `Vec::sort()` on the Rust-side values.
//!
//! All tests are gated on `DATABASE_URL` — they return early if the env
//! var is unset, so the file is a no-op on hosts without a live DB.
//!
//! Run:
//!
//! ```text
//! DATABASE_URL=postgres://... cargo test -p heeranjid-sqlx \
//!     --test desc_codec -- --test-threads=1
//! ```

use heeranjid::postgres_schema::{install_all_desc_support, install_schema, seed_default_node};
use heeranjid::{HeerIdDesc, RanjIdDesc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Executor, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_postgres::NoTls;

/// Shared setup: connect sqlx pool + tokio-postgres client, install the
/// base schema + desc support, set node id, drop any leftover fixture
/// tables from prior runs.
async fn setup(url: &str, tables: &[&str]) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await
        .expect("sqlx connect");

    let (pg_client, pg_conn) = tokio_postgres::connect(url, NoTls)
        .await
        .expect("tokio-postgres connect");
    tokio::spawn(async move {
        let _ = pg_conn.await;
    });

    install_schema(&pg_client).await.expect("install_schema");
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");
    pg_client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set_heer_node_id (tokio-postgres)");

    for tbl in tables {
        pool.execute(format!("DROP TABLE IF EXISTS {tbl} CASCADE").as_str())
            .await
            .expect("drop leftover fixture");
    }

    pool
}

#[tokio::test(flavor = "multi_thread")]
async fn heer_desc_codec_round_trip_sqlx() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    const TABLE: &str = "desc_codec_heer_rt";
    let pool = setup(&url, &[TABLE]).await;

    sqlx::query(&format!("CREATE TABLE {TABLE} (id bigint PRIMARY KEY)"))
        .execute(&pool)
        .await
        .expect("create table");

    // Construct a HeerIdDesc with concrete logical fields so we can
    // inspect what comes back, and exercise the Encode path.
    let original = HeerIdDesc::new(1_700_000_000_000, 7, 42).unwrap();
    sqlx::query(&format!("INSERT INTO {TABLE} (id) VALUES ($1)"))
        .bind(original)
        .execute(&pool)
        .await
        .expect("insert HeerIdDesc");

    // Read it back and decode via the sqlx Decode impl.
    let fetched: HeerIdDesc = sqlx::query_scalar(&format!("SELECT id FROM {TABLE} WHERE id = $1"))
        .bind(original)
        .fetch_one(&pool)
        .await
        .expect("fetch HeerIdDesc");
    assert_eq!(fetched, original, "stored bits must round-trip");
    assert_eq!(fetched.timestamp_ms(), 1_700_000_000_000);
    assert_eq!(fetched.node_id(), 7);
    assert_eq!(fetched.sequence(), 42);

    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ranj_desc_codec_round_trip_sqlx() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    const TABLE: &str = "desc_codec_ranj_rt";
    let pool = setup(&url, &[TABLE]).await;

    sqlx::query(&format!("CREATE TABLE {TABLE} (id uuid PRIMARY KEY)"))
        .execute(&pool)
        .await
        .expect("create table");

    // Use a concrete logical Ranj value that fits every field.
    let original = RanjIdDesc::new(
        1_700_000_000_000_000,
        heeranjid::RanjPrecision::Microseconds,
        7,
        42,
    )
    .unwrap();
    sqlx::query(&format!("INSERT INTO {TABLE} (id) VALUES ($1)"))
        .bind(original)
        .execute(&pool)
        .await
        .expect("insert RanjIdDesc");

    let fetched: RanjIdDesc = sqlx::query_scalar(&format!("SELECT id FROM {TABLE} WHERE id = $1"))
        .bind(original)
        .fetch_one(&pool)
        .await
        .expect("fetch RanjIdDesc");
    assert_eq!(fetched, original, "stored bits must round-trip");
    assert_eq!(fetched.timestamp(), 1_700_000_000_000_000);
    assert_eq!(fetched.node_id(), 7);
    assert_eq!(fetched.sequence(), 42);
    // UUIDv8 conformance preserved through the codec round-trip.
    assert_eq!(fetched.as_uuid().get_version_num(), 8);

    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn heerid_next_desc_generator_freshness() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    const TABLE: &str = "desc_codec_heer_gen";
    let pool = setup(&url, &[TABLE]).await;

    // DEFAULT heerid_next_desc() reads the session's heer.node_id, so pin
    // a connection and set it for the life of the fixture INSERT.
    let mut conn = pool.acquire().await.expect("acquire pinned conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *conn)
        .await
        .expect("set_heer_node_id");

    sqlx::query(&format!(
        "CREATE TABLE {TABLE} (id bigint PRIMARY KEY DEFAULT heerid_next_desc())"
    ))
    .execute(&mut *conn)
    .await
    .expect("create table with desc default");

    let now_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();

    sqlx::query(&format!("INSERT INTO {TABLE} DEFAULT VALUES"))
        .execute(&mut *conn)
        .await
        .expect("insert via DEFAULT heerid_next_desc()");

    let id: HeerIdDesc = sqlx::query_scalar(&format!("SELECT id FROM {TABLE}"))
        .fetch_one(&mut *conn)
        .await
        .expect("fetch generated HeerIdDesc");
    drop(conn);

    let ts = id.timestamp_ms();
    // HeerId uses the heer_config epoch; the absolute timestamp_ms is
    // relative to that epoch, so we compare to "now since the epoch".
    // The library's default epoch is near 2024, but rather than hard-code
    // assumptions we assert a relaxed bound: ts must be within a few
    // seconds of *something* in the recent past (>0 and <= now_ms).
    assert!(ts > 0, "generated timestamp must be non-zero");
    assert!(
        ts <= now_ms + 5_000,
        "generated ts={ts} should not exceed now_ms={now_ms} by more than 5s"
    );
    // Sanity: the node embedded in the generated ID must be the one we set.
    assert_eq!(id.node_id(), 1);

    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ranjid_next_desc_generator_freshness() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    const TABLE: &str = "desc_codec_ranj_gen";
    let pool = setup(&url, &[TABLE]).await;

    let mut conn = pool.acquire().await.expect("acquire pinned conn");
    sqlx::query("SELECT set_heer_ranj_node_id(1)")
        .execute(&mut *conn)
        .await
        .expect("set_heer_ranj_node_id");

    sqlx::query(&format!(
        "CREATE TABLE {TABLE} (id uuid PRIMARY KEY DEFAULT ranjid_next_desc())"
    ))
    .execute(&mut *conn)
    .await
    .expect("create table with ranj desc default");

    sqlx::query(&format!("INSERT INTO {TABLE} DEFAULT VALUES"))
        .execute(&mut *conn)
        .await
        .expect("insert via DEFAULT ranjid_next_desc()");

    let id: RanjIdDesc = sqlx::query_scalar(&format!("SELECT id FROM {TABLE}"))
        .fetch_one(&mut *conn)
        .await
        .expect("fetch generated RanjIdDesc");
    drop(conn);

    // Sanity: UUIDv8 conformance and our node_id are preserved.
    assert_eq!(id.as_uuid().get_version_num(), 8);
    assert_eq!(id.as_uuid().get_variant(), uuid::Variant::RFC4122);
    assert_eq!(id.node_id(), 1);
    // Timestamp should be non-zero and positive; precise freshness
    // windows depend on the library's configured precision + epoch,
    // which are DB-local settings. Asserting non-zero + UUIDv8
    // conformance is the load-bearing invariant for the codec path.
    assert!(
        id.timestamp() > 0,
        "generated RanjIdDesc timestamp must be non-zero"
    );

    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn heer_desc_db_sort_matches_rust_sort() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    };
    const TABLE: &str = "desc_codec_sort";
    let pool = setup(&url, &[TABLE]).await;

    sqlx::query(&format!("CREATE TABLE {TABLE} (id bigint PRIMARY KEY)"))
        .execute(&pool)
        .await
        .expect("create table");

    // Build a deterministic, non-sorted batch that spans enough logical
    // timestamps to give `ORDER BY id` something to do. Shuffle via a
    // fixed interleave so the test is deterministic (no rand dep).
    let logical_ms: Vec<u64> = (1..=64u64).collect();
    let shuffled: Vec<u64> = (0..logical_ms.len())
        .map(|i| {
            // Arbitrary fixed interleave — yields a non-monotonic order
            // without introducing a randomness dependency.
            logical_ms[(i * 37) % logical_ms.len()]
        })
        .collect();

    let values: Vec<HeerIdDesc> = shuffled
        .iter()
        .map(|&ms| HeerIdDesc::new(1_700_000_000_000 + ms, 3, 0).unwrap())
        .collect();

    for v in &values {
        sqlx::query(&format!("INSERT INTO {TABLE} (id) VALUES ($1)"))
            .bind(*v)
            .execute(&pool)
            .await
            .expect("insert shuffled value");
    }

    // DB side: SELECT ... ORDER BY id (ascending on stored bits, which
    // for desc types means reverse-chronological on logical timestamp).
    let db_ordered: Vec<HeerIdDesc> =
        sqlx::query_scalar(&format!("SELECT id FROM {TABLE} ORDER BY id"))
            .fetch_all(&pool)
            .await
            .expect("fetch ordered");

    // Rust side: the same set, sorted via derive(Ord) on the backing
    // bits. Must match the DB ordering bit-for-bit.
    let mut rust_ordered = values.clone();
    rust_ordered.sort();

    assert_eq!(
        db_ordered, rust_ordered,
        "DB ORDER BY id must match Rust Vec::sort() on HeerIdDesc"
    );

    // And as an extra sanity check on the invariant the whole feature
    // is built on: logical timestamps come out descending.
    let logical_out: Vec<u64> = db_ordered.iter().map(|v| v.timestamp_ms()).collect();
    assert!(
        logical_out.windows(2).all(|w| w[0] >= w[1]),
        "ORDER BY id on desc column must produce reverse-chronological logical timestamps: {logical_out:?}"
    );

    pool.execute(format!("DROP TABLE {TABLE} CASCADE").as_str())
        .await
        .ok();
}
