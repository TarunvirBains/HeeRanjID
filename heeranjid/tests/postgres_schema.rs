//! Schema installation and seed tests for the `postgres` feature.
//!
//! Tests exercise `install_schema()` and `seed_default_node()` helpers
//! against a real Postgres instance, verifying that the DDL is idempotent
//! and that the seed creates the expected default node row.
//!
//! Requires a running Postgres instance reachable via the `DATABASE_URL`
//! environment variable. If unset, tests are skipped (printed to stderr).
//! The test suite uses `tokio::test` for async execution and `tokio-postgres`
//! with `NoTls` for the connection.
//!
//! Tests are compiled only when the `postgres` feature is enabled.

#![cfg(feature = "postgres")]

use std::env;
use tokio_postgres::NoTls;

async fn connect() -> Option<tokio_postgres::Client> {
    let url = env::var("DATABASE_URL").ok()?;
    let (client, conn) = tokio_postgres::connect(&url, NoTls).await.ok()?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("postgres connection error: {e}");
        }
    });
    Some(client)
}

// ---------------------------------------------------------------------------
// Schema installation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn install_schema_creates_tables() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    // Create an isolated schema for testing.
    let schema_name = "test_heeranjid_install";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");

    // Set search_path so subsequent DDL lands in our isolated schema.
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    // Run install_schema.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema should succeed");

    // Verify core tables exist.
    let tables: Vec<String> = client
        .query_opt(
            "SELECT tablename FROM pg_tables WHERE schemaname = $1 AND tablename = 'heer_nodes'",
            &[&schema_name],
        )
        .await
        .expect("query pg_tables")
        .iter()
        .map(|row| row.get(0))
        .collect();

    assert!(
        !tables.is_empty(),
        "heer_nodes table should exist after install"
    );

    // Re-run install_schema to verify idempotency.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema should be idempotent");

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// Seed installation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn seed_default_node_creates_row() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    // Create an isolated schema for testing.
    let schema_name = "test_heeranjid_seed";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");

    // Set search_path so subsequent DDL lands in our isolated schema.
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    // Install schema first.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");

    // Seed default node.
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node should succeed");

    // Verify default node (node_id = 1) exists.
    let count: i64 = client
        .query_one("SELECT count(*) FROM heer_nodes WHERE node_id = 1", &[])
        .await
        .expect("query heer_nodes")
        .get(0);

    assert_eq!(
        count, 1,
        "default node (node_id = 1) should exist after seed"
    );

    // Re-run seed to verify idempotency.
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node should be idempotent");

    // Verify count is still 1 (not duplicated).
    let count_after_reseed: i64 = client
        .query_one("SELECT count(*) FROM heer_nodes WHERE node_id = 1", &[])
        .await
        .expect("query heer_nodes after reseed")
        .get(0);

    assert_eq!(
        count_after_reseed, 1,
        "default node should not be duplicated on re-seed"
    );

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// Desc flip round-trip (install_all_desc_support)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn desc_flip_round_trips_inside_postgres() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    // Create an isolated schema for testing.
    let schema_name = "test_heeranjid_desc_flip";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");

    // Set search_path so subsequent DDL lands in our isolated schema.
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    // Install schema, seed, and all desc support.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node");
    heeranjid::postgres_schema::install_all_desc_support(&client)
        .await
        .expect("install_all_desc_support");

    // heerid_to_asc(heerid_to_desc(1234567)) must round-trip.
    let row = client
        .query_one("SELECT heerid_to_asc(heerid_to_desc(1234567::bigint))", &[])
        .await
        .expect("round-trip query");
    let back: i64 = row.get(0);
    assert_eq!(back, 1_234_567, "heerid_to_asc(heerid_to_desc(x)) == x");

    // heerid_flip_mask() must equal the documented constant.
    let row = client
        .query_one("SELECT heerid_flip_mask()", &[])
        .await
        .expect("flip mask query");
    let mask: i64 = row.get(0);
    assert_eq!(
        mask, 9_223_372_036_850_589_695,
        "heerid_flip_mask() == documented constant"
    );

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// ID generation post-seed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_id_after_seed() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    // Create an isolated schema for testing.
    let schema_name = "test_heeranjid_genid";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");

    // Set search_path so subsequent DDL lands in our isolated schema.
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    // Install schema and seed.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node");

    // Set session node_id for generation.
    client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set session node_id");

    // Generate an ID.
    let id: i64 = client
        .query_one("SELECT generate_id()", &[])
        .await
        .expect("generate_id")
        .get(0);

    assert!(id > 0, "generated ID should be positive");

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// Per-table autofill trigger (Task 11)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn autofill_trigger_populates_desc_column_on_insert_and_update() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    // Create an isolated schema for testing.
    let schema_name = "test_heeranjid_autofill_trigger";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");

    // Pin search_path so all DDL and the trigger body resolve here.
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    // Install schema + all desc support (flip fns are what the trigger calls).
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node");
    heeranjid::postgres_schema::install_all_desc_support(&client)
        .await
        .expect("install_all_desc_support");

    // Fixture table: plain int64 pk + a sibling desc column.
    client
        .batch_execute("CREATE TABLE trig_test (id bigint PRIMARY KEY, id_desc bigint)")
        .await
        .expect("create trig_test");

    // Install the per-table trigger (single pair, Heer kind).
    heeranjid::postgres_schema::install_autofill_trigger_for_table(
        &client,
        "trig_test",
        &[heeranjid::postgres_schema::ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        heeranjid::postgres_schema::IdKind::Heer,
    )
    .await
    .expect("install_autofill_trigger_for_table");

    // INSERT without populating id_desc — trigger must fill it.
    client
        .execute("INSERT INTO trig_test (id) VALUES ($1)", &[&1000_i64])
        .await
        .expect("insert row");

    let expected: i64 = client
        .query_one("SELECT heerid_to_desc($1::bigint)", &[&1000_i64])
        .await
        .expect("expected id_desc for 1000")
        .get(0);
    let got: i64 = client
        .query_one("SELECT id_desc FROM trig_test WHERE id = $1", &[&1000_i64])
        .await
        .expect("read id_desc after insert")
        .get(0);
    assert_eq!(
        got, expected,
        "INSERT trigger should populate id_desc via heerid_to_desc(id)"
    );

    // UPDATE the source — trigger must recompute id_desc.
    client
        .execute(
            "UPDATE trig_test SET id = $1 WHERE id = $2",
            &[&2000_i64, &1000_i64],
        )
        .await
        .expect("update row");

    let expected2: i64 = client
        .query_one("SELECT heerid_to_desc($1::bigint)", &[&2000_i64])
        .await
        .expect("expected id_desc for 2000")
        .get(0);
    let got2: i64 = client
        .query_one("SELECT id_desc FROM trig_test WHERE id = $1", &[&2000_i64])
        .await
        .expect("read id_desc after update")
        .get(0);
    assert_eq!(
        got2, expected2,
        "UPDATE trigger should recompute id_desc when source changes"
    );

    // Drop the trigger and confirm it's gone.
    heeranjid::postgres_schema::drop_autofill_trigger_for_table(&client, "trig_test")
        .await
        .expect("drop_autofill_trigger_for_table");

    let remaining: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_trigger \
             WHERE tgname = 'zzz_trig_test_autofill_desc' AND NOT tgisinternal",
            &[],
        )
        .await
        .expect("check trigger removal")
        .get(0);
    assert_eq!(remaining, 0, "trigger should be gone after drop helper");

    // After drop, an UPDATE to id must NOT touch id_desc.
    client
        .execute(
            "UPDATE trig_test SET id = $1 WHERE id = $2",
            &[&3000_i64, &2000_i64],
        )
        .await
        .expect("update row post-drop");
    let stale: i64 = client
        .query_one("SELECT id_desc FROM trig_test WHERE id = $1", &[&3000_i64])
        .await
        .expect("read id_desc after post-drop update")
        .get(0);
    assert_eq!(
        stale, expected2,
        "after drop, id_desc must not be recomputed by a trigger"
    );

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// Bulk descending generators (v0.3.4)
// ---------------------------------------------------------------------------
//
// `generate_ids_desc(n)` and `generate_ranjids_desc(n)` are the batch
// counterparts to `heerid_next_desc()` / `ranjid_next_desc()`. They compose
// the existing asc allocator with the desc flip so callers get a column of
// descending-shape IDs in a single round-trip, without reaching for the
// flip primitives directly.
//
// These tests verify, for both HeerId and RanjId:
//   a) the function returns exactly the requested row count;
//   b) each returned desc-shape ID decodes back to a valid asc ID via the
//      matching `*_to_asc` primitive (self-inverse XOR);
//   c) the returned IDs are distinct.

#[tokio::test]
async fn generate_ids_desc_returns_flipped_batch() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_bulk_heerid_desc";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node");
    heeranjid::postgres_schema::install_all_desc_support(&client)
        .await
        .expect("install_all_desc_support");

    // Pin the session node so the `requested_count`-only overload resolves.
    client
        .execute("SELECT set_heer_node_id(1)", &[])
        .await
        .expect("set_heer_node_id");

    let requested: i32 = 8;

    // (a) Row count: the one-arg overload returns `requested` rows.
    let desc_rows = client
        .query(
            "SELECT id FROM generate_ids_desc($1::integer)",
            &[&requested],
        )
        .await
        .expect("bulk generate_ids_desc");
    assert_eq!(
        desc_rows.len(),
        requested as usize,
        "generate_ids_desc($1) must return exactly $1 rows"
    );

    let desc_ids: Vec<i64> = desc_rows.iter().map(|r| r.get::<_, i64>(0)).collect();

    // (b) Flip actually happened: each desc ID equals heerid_to_desc(asc).
    // Re-derive what the desc values *should* be by flipping each asc-shape
    // back through heerid_to_desc, and assert equality. A wrapper that
    // accidentally returned raw asc IDs would fail here because
    // heerid_to_desc(asc) != asc for any real HeerId.
    for d in &desc_ids {
        let roundtrip_row = client
            .query_one(
                "SELECT heerid_to_desc(heerid_to_asc($1::bigint))",
                &[d],
            )
            .await
            .expect("heerid_to_desc(heerid_to_asc(d)) round-trip");
        let roundtrip: i64 = roundtrip_row.get(0);
        assert_eq!(
            *d, roundtrip,
            "heerid_to_desc(heerid_to_asc(d)) must equal d — wrapper must apply the flip"
        );
    }

    // (c) Flip is self-inverse: desc -> asc -> each asc value must decode
    // as a valid HeerId, and the asc sequence must be strictly monotonic
    // increasing (which would break if the wrapper returned already-flipped
    // values and heerid_to_asc then double-flipped them into non-monotonic
    // noise).
    let mut asc_ids: Vec<i64> = Vec::with_capacity(desc_ids.len());
    for d in &desc_ids {
        let asc_row = client
            .query_one("SELECT heerid_to_asc($1::bigint)", &[d])
            .await
            .expect("flip back to asc");
        let asc: i64 = asc_row.get(0);
        heeranjid::HeerId::from_i64(asc)
            .expect("asc-shape round-trip must parse as a valid HeerId");
        asc_ids.push(asc);
    }
    for window in asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "asc-flipped sequence must be strictly monotonic increasing; \
             got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

    // (d) Distinctness: the batch must contain no duplicates.
    let mut sorted = desc_ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        desc_ids.len(),
        "generate_ids_desc must return distinct IDs"
    );

    // (e) Explicit-node overload (`(in_node_id, requested_count, spanning)`)
    // must also honour the row count and apply the flip.
    let node_rows = client
        .query(
            "SELECT id FROM generate_ids_desc($1::integer, $2::integer, true)",
            &[&1_i32, &requested],
        )
        .await
        .expect("bulk generate_ids_desc with explicit node");
    assert_eq!(
        node_rows.len(),
        requested as usize,
        "generate_ids_desc(node, n, spanning) must return n rows"
    );
    for row in &node_rows {
        let d: i64 = row.get(0);
        let rt_row = client
            .query_one(
                "SELECT heerid_to_desc(heerid_to_asc($1::bigint))",
                &[&d],
            )
            .await
            .expect("flip verification for explicit-node overload");
        let rt: i64 = rt_row.get(0);
        assert_eq!(d, rt, "explicit-node overload must apply the desc flip");
    }

    // (f) allow_spanning=false variant: 2-arg session-node overload.
    // Requesting 1 ID with spanning disabled must succeed.
    let no_span_rows = client
        .query(
            "SELECT id FROM generate_ids_desc($1::integer, $2::boolean)",
            &[&1_i32, &false],
        )
        .await
        .expect("generate_ids_desc(1, false)");
    assert_eq!(
        no_span_rows.len(),
        1,
        "generate_ids_desc(n, false) must return n rows"
    );
    let no_span_id: i64 = no_span_rows[0].get(0);
    let rt_row = client
        .query_one(
            "SELECT heerid_to_desc(heerid_to_asc($1::bigint))",
            &[&no_span_id],
        )
        .await
        .expect("flip verification for allow_spanning=false");
    let rt: i64 = rt_row.get(0);
    assert_eq!(
        no_span_id, rt,
        "allow_spanning=false overload must apply the desc flip"
    );

    // (g) Zero-count propagates the underlying error.
    let zero_err = client
        .query("SELECT id FROM generate_ids_desc($1::integer)", &[&0_i32])
        .await;
    assert!(
        zero_err.is_err(),
        "generate_ids_desc(0) must propagate requested_count error"
    );
    let pg_err = zero_err.unwrap_err();
    let db_err = pg_err
        .as_db_error()
        .expect("generate_ids_desc(0) must raise a Postgres-level error");
    assert!(
        db_err.message().contains("requested_count must be greater than zero"),
        "error message must mention requested_count; got: {}",
        db_err.message()
    );

    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

#[tokio::test]
async fn generate_ranjids_desc_returns_flipped_batch() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_bulk_ranjid_desc";
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {schema_name}"), &[])
        .await
        .expect("create test schema");
    client
        .execute(&format!("SET search_path TO {schema_name}"), &[])
        .await
        .expect("set search_path");

    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(&client)
        .await
        .expect("seed_default_node");
    heeranjid::postgres_schema::install_all_desc_support(&client)
        .await
        .expect("install_all_desc_support");

    // Pin the session ranj node so the one-arg overload resolves.
    client
        .execute("SELECT set_heer_ranj_node_id(1)", &[])
        .await
        .expect("set_heer_ranj_node_id");

    let requested: i32 = 8;

    // (a) Row count.
    let desc_rows = client
        .query(
            "SELECT id FROM generate_ranjids_desc($1::integer)",
            &[&requested],
        )
        .await
        .expect("bulk generate_ranjids_desc");
    assert_eq!(
        desc_rows.len(),
        requested as usize,
        "generate_ranjids_desc($1) must return exactly $1 rows"
    );

    let desc_ids: Vec<uuid::Uuid> = desc_rows
        .iter()
        .map(|r| r.get::<_, uuid::Uuid>(0))
        .collect();

    // (b) Flip actually happened: each desc ID equals ranjid_to_desc(asc).
    // A wrapper that forgot the flip and returned raw asc IDs would produce
    // ranjid_to_desc(ranjid_to_asc(d)) != d, failing this assertion.
    for d in &desc_ids {
        let roundtrip_row = client
            .query_one(
                "SELECT ranjid_to_desc(ranjid_to_asc($1::uuid))",
                &[d],
            )
            .await
            .expect("ranjid_to_desc(ranjid_to_asc(d)) round-trip");
        let roundtrip: uuid::Uuid = roundtrip_row.get(0);
        assert_eq!(
            *d, roundtrip,
            "ranjid_to_desc(ranjid_to_asc(d)) must equal d — wrapper must apply the flip"
        );
    }

    // (c) Flip is self-inverse: desc -> asc -> each asc value must decode as
    // a valid RanjId, and the asc sequence must be strictly monotonic
    // increasing (catches a wrapper that double-flips into non-monotonic noise).
    let mut asc_ids: Vec<uuid::Uuid> = Vec::with_capacity(desc_ids.len());
    for d in &desc_ids {
        let asc_row = client
            .query_one("SELECT ranjid_to_asc($1::uuid)", &[d])
            .await
            .expect("flip back to asc");
        let asc: uuid::Uuid = asc_row.get(0);
        heeranjid::RanjId::from_uuid(asc)
            .expect("asc-shape round-trip must parse as a valid RanjId");
        asc_ids.push(asc);
    }
    for window in asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "asc-flipped sequence must be strictly monotonic increasing; \
             got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

    // (d) Distinctness.
    let mut sorted = desc_ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        desc_ids.len(),
        "generate_ranjids_desc must return distinct IDs"
    );

    // (e) Explicit-node overload (`(in_node_id, requested_count, spanning)`)
    // must also honour row count and apply the flip.
    let node_rows = client
        .query(
            "SELECT id FROM generate_ranjids_desc($1::integer, $2::integer, true)",
            &[&1_i32, &requested],
        )
        .await
        .expect("bulk generate_ranjids_desc with explicit node");
    assert_eq!(
        node_rows.len(),
        requested as usize,
        "generate_ranjids_desc(node, n, spanning) must return n rows"
    );
    for row in &node_rows {
        let d: uuid::Uuid = row.get(0);
        let rt_row = client
            .query_one(
                "SELECT ranjid_to_desc(ranjid_to_asc($1::uuid))",
                &[&d],
            )
            .await
            .expect("flip verification for explicit-node overload");
        let rt: uuid::Uuid = rt_row.get(0);
        assert_eq!(d, rt, "explicit-node overload must apply the desc flip");
    }

    // (f) allow_spanning=false variant: 2-arg session-node overload.
    let no_span_rows = client
        .query(
            "SELECT id FROM generate_ranjids_desc($1::integer, $2::boolean)",
            &[&1_i32, &false],
        )
        .await
        .expect("generate_ranjids_desc(1, false)");
    assert_eq!(
        no_span_rows.len(),
        1,
        "generate_ranjids_desc(n, false) must return n rows"
    );
    let no_span_id: uuid::Uuid = no_span_rows[0].get(0);
    let rt_row = client
        .query_one(
            "SELECT ranjid_to_desc(ranjid_to_asc($1::uuid))",
            &[&no_span_id],
        )
        .await
        .expect("flip verification for allow_spanning=false");
    let rt: uuid::Uuid = rt_row.get(0);
    assert_eq!(
        no_span_id, rt,
        "allow_spanning=false overload must apply the desc flip"
    );

    // (g) Zero-count propagates the underlying error.
    let zero_err = client
        .query(
            "SELECT id FROM generate_ranjids_desc($1::integer)",
            &[&0_i32],
        )
        .await;
    assert!(
        zero_err.is_err(),
        "generate_ranjids_desc(0) must propagate requested_count error"
    );
    let pg_err = zero_err.unwrap_err();
    let db_err = pg_err
        .as_db_error()
        .expect("generate_ranjids_desc(0) must raise a Postgres-level error");
    assert!(
        db_err.message().contains("requested_count must be greater than zero"),
        "error message must mention requested_count; got: {}",
        db_err.message()
    );

    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}
