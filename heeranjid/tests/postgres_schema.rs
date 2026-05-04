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

    // (b) Involutive property: heerid_to_desc(heerid_to_asc(d)) == d for every
    // returned id. Because both flip functions are the same XOR-mask operation,
    // applying them in sequence is a no-op on any value — this is a tautology
    // that documents the involutive (self-inverse) property of the flip
    // functions, NOT a check that the wrapper applied the flip. A wrapper that
    // returned raw asc IDs would still pass this assertion. The monotonicity
    // check in (c) is what actually catches a missed flip.
    for d in &desc_ids {
        let roundtrip_row = client
            .query_one("SELECT heerid_to_desc(heerid_to_asc($1::bigint))", &[d])
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
    // must honour the row count and apply the flip. Monotonicity of the
    // asc-flipped sequence is the real flip-detection: if the wrapper forgot
    // heerid_to_desc, heerid_to_asc would produce a *decreasing* sequence.
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
    let mut node_asc_ids: Vec<i64> = Vec::with_capacity(node_rows.len());
    for row in &node_rows {
        let d: i64 = row.get(0);
        let asc_row = client
            .query_one("SELECT heerid_to_asc($1::bigint)", &[&d])
            .await
            .expect("flip back to asc — explicit-node overload");
        let asc: i64 = asc_row.get(0);
        heeranjid::HeerId::from_i64(asc)
            .expect("asc-shape must parse as a valid HeerId — explicit-node overload");
        node_asc_ids.push(asc);
    }
    for window in node_asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "explicit-node overload: asc-flipped sequence must be strictly monotonic increasing; \
             got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

    // (f) allow_spanning=false variant: 2-arg session-node overload.
    // Request enough IDs to verify monotonicity (a single ID cannot establish
    // ordering, so request `requested` IDs here too).
    let no_span_rows = client
        .query(
            "SELECT id FROM generate_ids_desc($1::integer, $2::boolean)",
            &[&requested, &false],
        )
        .await
        .expect("generate_ids_desc(n, false)");
    assert_eq!(
        no_span_rows.len(),
        requested as usize,
        "generate_ids_desc(n, false) must return n rows"
    );
    let mut no_span_asc_ids: Vec<i64> = Vec::with_capacity(no_span_rows.len());
    for row in &no_span_rows {
        let d: i64 = row.get(0);
        let asc_row = client
            .query_one("SELECT heerid_to_asc($1::bigint)", &[&d])
            .await
            .expect("flip back to asc — allow_spanning=false overload");
        let asc: i64 = asc_row.get(0);
        heeranjid::HeerId::from_i64(asc)
            .expect("asc-shape must parse as a valid HeerId — allow_spanning=false overload");
        no_span_asc_ids.push(asc);
    }
    for window in no_span_asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "allow_spanning=false overload: asc-flipped sequence must be strictly monotonic \
             increasing; got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

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
        db_err
            .message()
            .contains("requested_count must be greater than zero"),
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

    // (b) Involutive property: ranjid_to_desc(ranjid_to_asc(d)) == d for every
    // returned id. Because both flip functions apply the same XOR-mask, applying
    // them in sequence is a no-op on any value — this is a tautology documenting
    // the involutive (self-inverse) property of the flip functions, NOT a check
    // that the wrapper applied the flip. A wrapper that returned raw asc IDs
    // would still pass. The monotonicity check in (c) is what catches a missed
    // flip.
    for d in &desc_ids {
        let roundtrip_row = client
            .query_one("SELECT ranjid_to_desc(ranjid_to_asc($1::uuid))", &[d])
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
    // must honour the row count and apply the flip. Monotonicity of the
    // asc-flipped sequence is the real flip-detection: if the wrapper forgot
    // ranjid_to_desc, ranjid_to_asc would produce a *decreasing* sequence.
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
    let mut node_asc_ids: Vec<uuid::Uuid> = Vec::with_capacity(node_rows.len());
    for row in &node_rows {
        let d: uuid::Uuid = row.get(0);
        let asc_row = client
            .query_one("SELECT ranjid_to_asc($1::uuid)", &[&d])
            .await
            .expect("flip back to asc — explicit-node overload");
        let asc: uuid::Uuid = asc_row.get(0);
        heeranjid::RanjId::from_uuid(asc)
            .expect("asc-shape must parse as a valid RanjId — explicit-node overload");
        node_asc_ids.push(asc);
    }
    for window in node_asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "explicit-node overload: asc-flipped sequence must be strictly monotonic increasing; \
             got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

    // (f) allow_spanning=false variant: 2-arg session-node overload.
    // Request enough IDs to verify monotonicity (a single ID cannot establish
    // ordering, so request `requested` IDs here too).
    let no_span_rows = client
        .query(
            "SELECT id FROM generate_ranjids_desc($1::integer, $2::boolean)",
            &[&requested, &false],
        )
        .await
        .expect("generate_ranjids_desc(n, false)");
    assert_eq!(
        no_span_rows.len(),
        requested as usize,
        "generate_ranjids_desc(n, false) must return n rows"
    );
    let mut no_span_asc_ids: Vec<uuid::Uuid> = Vec::with_capacity(no_span_rows.len());
    for row in &no_span_rows {
        let d: uuid::Uuid = row.get(0);
        let asc_row = client
            .query_one("SELECT ranjid_to_asc($1::uuid)", &[&d])
            .await
            .expect("flip back to asc — allow_spanning=false overload");
        let asc: uuid::Uuid = asc_row.get(0);
        heeranjid::RanjId::from_uuid(asc)
            .expect("asc-shape must parse as a valid RanjId — allow_spanning=false overload");
        no_span_asc_ids.push(asc);
    }
    for window in no_span_asc_ids.windows(2) {
        assert!(
            window[0] < window[1],
            "allow_spanning=false overload: asc-flipped sequence must be strictly monotonic \
             increasing; got {} then {} — wrapper may have skipped the desc flip",
            window[0],
            window[1],
        );
    }

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
        db_err
            .message()
            .contains("requested_count must be greater than zero"),
        "error message must mention requested_count; got: {}",
        db_err.message()
    );

    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// install_configure / heer_configure() (issue #40)
// ---------------------------------------------------------------------------
//
// Verifies that `install_configure()` installs the `heer_configure()` stored
// procedure and that calling it succeeds end-to-end without error.  Requires a
// live `heer_config` row so the smoke test inside `heer_configure()` can run
// `generate_id(1)` / `generate_ranjid(1)`.

#[tokio::test]
async fn install_configure_and_call_heer_configure() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_configure";
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

    // Base schema + functions.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");

    // Manual seed — avoids the ON CONFLICT DO NOTHING in seed_default_node()
    // conflicting with the precision-specific epoch row inserted below.
    client
        .execute(
            "INSERT INTO heer_config (id, epoch, precision) \
             VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day', 'us')",
            &[],
        )
        .await
        .expect("insert heer_config");
    client
        .execute(
            "INSERT INTO heer_nodes (node_id, name, description, is_active) \
             VALUES (1, 'default', 'Default single-node instance', true)",
            &[],
        )
        .await
        .expect("insert heer_nodes");
    client
        .execute("INSERT INTO heer_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_node_state");
    client
        .execute("INSERT INTO heer_ranj_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_ranj_node_state");

    // Install the heer_configure() stored procedure.
    heeranjid::postgres_schema::install_configure(&client)
        .await
        .expect("install_configure should succeed");

    // Call heer_configure() — this validates config, regenerates generate_ids /
    // generate_ranjids with baked-in constants, resets node state, and runs a
    // smoke test.  Any error here indicates a bug in configure.sql.
    client
        .execute("SELECT heer_configure()", &[])
        .await
        .expect("heer_configure() should succeed without error");

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema (configure)");
}

// ---------------------------------------------------------------------------
// Decoded RanjId timestamp (issue #40 / issue #33)
// ---------------------------------------------------------------------------
//
// Generates a RanjId via the embedded SQL path (`generate_ranjid(1)`), then
// decodes it with `RanjId::from_uuid()` / `RanjId::timestamp_micros()` and
// asserts that the decoded timestamp is within 5 seconds of the current wall
// clock.  Before the precision_bits fix in issue #33 the decoded timestamp was
// 1000x too small (nanoseconds stored as microseconds), so this test would
// have failed with a value close to `now / 1000`.

#[tokio::test]
async fn decoded_ranjid_timestamp_is_current() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_ranjid_ts";
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

    // Manual seed — precision 'us' is required for the timestamp decode check;
    // seed_default_node() inserts 'ns' and ON CONFLICT DO NOTHING would silently
    // leave the wrong precision, or a plain INSERT would fail with a duplicate key.
    client
        .execute(
            "INSERT INTO heer_config (id, epoch, precision) \
             VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day', 'us')",
            &[],
        )
        .await
        .expect("insert heer_config");
    client
        .execute(
            "INSERT INTO heer_nodes (node_id, name, description, is_active) \
             VALUES (1, 'default', 'Default single-node instance', true)",
            &[],
        )
        .await
        .expect("insert heer_nodes");
    client
        .execute("INSERT INTO heer_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_node_state");
    client
        .execute("INSERT INTO heer_ranj_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_ranj_node_state");

    // Capture wall time just before generation so we can bound the timestamp.
    let before_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time is after epoch")
        .as_micros();

    // Generate via the embedded SQL path.
    let uuid: uuid::Uuid = client
        .query_one("SELECT generate_ranjid(1)", &[])
        .await
        .expect("generate_ranjid(1)")
        .get(0);

    let after_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time is after epoch")
        .as_micros();

    let ranj = heeranjid::RanjId::from_uuid(uuid).expect("database returned a valid RanjId UUID");

    // The RanjId tick counts microseconds since the configured epoch
    // (CURRENT_TIMESTAMP - 1 day).  Convert to Unix microseconds by adding
    // the epoch offset.
    let epoch_micros: u128 = client
        .query_one(
            "SELECT FLOOR(EXTRACT(EPOCH FROM epoch) * 1000000)::BIGINT \
             FROM heer_config WHERE id = 1",
            &[],
        )
        .await
        .expect("fetch epoch_micros")
        .get::<_, i64>(0) as u128;

    let decoded_unix_micros = epoch_micros + ranj.timestamp_micros();

    const TOLERANCE_MICROS: u128 = 5_000_000; // 5 seconds
    assert!(
        decoded_unix_micros >= before_micros.saturating_sub(TOLERANCE_MICROS),
        "decoded timestamp {} µs is more than 5 s before generation start {} µs",
        decoded_unix_micros,
        before_micros,
    );
    assert!(
        decoded_unix_micros <= after_micros + TOLERANCE_MICROS,
        "decoded timestamp {} µs is more than 5 s after generation end {} µs",
        decoded_unix_micros,
        after_micros,
    );

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema (ranjid_ts)");
}

// ---------------------------------------------------------------------------
// Configured-path rollback SQLSTATE (issue #40)
// ---------------------------------------------------------------------------
//
// After `heer_configure()` activates the configured generation path, seeding
// `last_id_time` far in the future must still surface the typed
// `HardClockRollback` error.  This is the configured-path parallel of the
// existing `generate_heerid_surfaces_typed_rollback` /
// `generate_ranjid_surfaces_hard_clock_rollback` tests in
// `postgres_generate.rs`.

#[tokio::test]
async fn configured_ranjid_path_surfaces_hard_clock_rollback() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_configured_rollback";
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

    // Manual seed — avoids the ON CONFLICT DO NOTHING in seed_default_node()
    // conflicting with the precision-specific epoch row inserted below.
    client
        .execute(
            "INSERT INTO heer_config (id, epoch, precision) \
             VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day', 'us')",
            &[],
        )
        .await
        .expect("insert heer_config");
    client
        .execute(
            "INSERT INTO heer_nodes (node_id, name, description, is_active) \
             VALUES (1, 'default', 'Default single-node instance', true)",
            &[],
        )
        .await
        .expect("insert heer_nodes");
    client
        .execute("INSERT INTO heer_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_node_state");
    client
        .execute("INSERT INTO heer_ranj_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_ranj_node_state");

    // Activate the configured path.
    heeranjid::postgres_schema::install_configure(&client)
        .await
        .expect("install_configure");
    client
        .execute("SELECT heer_configure()", &[])
        .await
        .expect("heer_configure() should succeed");

    // Seed last_id_time far in the future (999 trillion ticks) to trigger hard
    // clock rollback on the next generation call, regardless of execution latency.
    client
        .execute(
            "INSERT INTO heer_ranj_node_state (node_id, last_id_time, last_sequence) \
             VALUES (1, 999999999999999, 0) \
             ON CONFLICT (node_id) DO UPDATE \
             SET last_id_time = EXCLUDED.last_id_time, \
                 last_sequence = EXCLUDED.last_sequence",
            &[],
        )
        .await
        .expect("seed heer_ranj_node_state with future timestamp");

    // The typed generate helper must surface HardClockRollback.
    let error = heeranjid::postgres_generate::generate_ranjid(&client, 1)
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            heeranjid::postgres_generate::GenerateError::HardClockRollback { .. }
        ),
        "expected HardClockRollback on configured path, got {:?}",
        error,
    );

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema (configured_rollback)");
}

// ---------------------------------------------------------------------------
// Upgrade identity hazard: old zero-arg overload is dropped by install_configure
// ---------------------------------------------------------------------------
//
// When upgrading from a schema that has an old zero-arg `heer_configure()`
// (pre-BOOLEAN-parameter version), `install_configure()` must drop that overload
// before creating the new one.  Without the DROP FUNCTION IF EXISTS line in
// configure.sql, the old overload would shadow or conflict with the new one.
//
// Steps:
//   1. Fresh schema + install_schema() + manual seed.
//   2. Create the old zero-arg overload manually (raises an exception if called).
//   3. Call install_configure() — the DROP FUNCTION IF EXISTS heer_configure()
//      line in configure.sql must remove the old overload first.
//   4. Call SELECT heer_configure() — must succeed (new BOOLEAN overload with
//      default), not raise 'old overload still present'.
//   5. Call SELECT heer_configure(false) — must also succeed.

#[tokio::test]
async fn heer_configure_upgrade_drops_old_overload() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_configure_upgrade";
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

    // Step 1: Install base schema.
    heeranjid::postgres_schema::install_schema(&client)
        .await
        .expect("install_schema");

    // Manual seed.
    client
        .execute(
            "INSERT INTO heer_config (id, epoch, precision) \
             VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day', 'us')",
            &[],
        )
        .await
        .expect("insert heer_config");
    client
        .execute(
            "INSERT INTO heer_nodes (node_id, name, description, is_active) \
             VALUES (1, 'default', 'Default single-node instance', true)",
            &[],
        )
        .await
        .expect("insert heer_nodes");
    client
        .execute("INSERT INTO heer_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_node_state");
    client
        .execute("INSERT INTO heer_ranj_node_state (node_id) VALUES (1)", &[])
        .await
        .expect("insert heer_ranj_node_state");

    // Step 2: Create the old zero-arg overload that would exist in a pre-upgrade schema.
    client
        .batch_execute(
            "CREATE FUNCTION heer_configure() RETURNS VOID LANGUAGE plpgsql AS $$ \
             BEGIN RAISE EXCEPTION 'old overload still present'; END; $$",
        )
        .await
        .expect("create old zero-arg heer_configure overload");

    // Step 3: install_configure() must DROP the old overload before creating the new one.
    heeranjid::postgres_schema::install_configure(&client)
        .await
        .expect("install_configure should succeed and drop the old overload");

    // Step 4: Calling heer_configure() (zero args, resolved via default) must invoke
    // the new BOOLEAN overload, not raise 'old overload still present'.
    client
        .execute("SELECT heer_configure()", &[])
        .await
        .expect("heer_configure() must call the new overload, not the old zero-arg one");

    // Step 5: Explicit-false variant must also succeed.
    client
        .execute("SELECT heer_configure(false)", &[])
        .await
        .expect("heer_configure(false) should succeed");

    // Cleanup.
    client
        .execute(&format!("DROP SCHEMA {schema_name} CASCADE"), &[])
        .await
        .expect("drop test schema (configure_upgrade)");
}
