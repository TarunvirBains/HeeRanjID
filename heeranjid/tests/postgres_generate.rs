//! Generation helper tests for the `postgres` feature.
//!
//! Exercises the four `postgres_generate` helpers
//! (`generate_heerid`, `generate_ranjid`, `generate_heerids`,
//! `generate_ranjids`) against a real Postgres instance. Each test
//! installs the schema into an isolated per-test schema, seeds the
//! default node, then calls the helper under test and asserts the
//! returned values round-trip through `HeerId::from_i64` /
//! `RanjId::from_uuid`.
//!
//! Requires a running Postgres instance reachable via the
//! `DATABASE_URL` environment variable. If unset, tests skip with a
//! printed warning (not a failure). Uses `tokio-postgres` with `NoTls`
//! for the connection, matching the sibling `postgres_schema.rs` test
//! suite.
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

/// Creates an isolated schema, sets `search_path`, installs the
/// HeeRanjID schema, and seeds the default node. Returns the schema
/// name so the caller can `DROP SCHEMA ... CASCADE` at teardown.
async fn setup_schema(client: &tokio_postgres::Client, name: &str) {
    client
        .execute(&format!("DROP SCHEMA IF EXISTS {name} CASCADE"), &[])
        .await
        .expect("drop test schema");
    client
        .execute(&format!("CREATE SCHEMA {name}"), &[])
        .await
        .expect("create test schema");
    client
        .execute(&format!("SET search_path TO {name}"), &[])
        .await
        .expect("set search_path");
    heeranjid::postgres_schema::install_schema(client)
        .await
        .expect("install_schema");
    heeranjid::postgres_schema::seed_default_node(client)
        .await
        .expect("seed_default_node");
}

async fn teardown_schema(client: &tokio_postgres::Client, name: &str) {
    client
        .execute(&format!("DROP SCHEMA {name} CASCADE"), &[])
        .await
        .expect("drop test schema");
}

// ---------------------------------------------------------------------------
// Single-row HeerId generation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_heerid_returns_valid_id() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_generate_heerid";
    setup_schema(&client, schema_name).await;

    let id = heeranjid::postgres_generate::generate_heerid(&client, 1)
        .await
        .expect("generate_heerid should succeed");

    // Round-trip: the returned HeerId must serialise to an i64 that
    // parses back to the same value via from_i64.
    let raw = id.as_i64();
    assert!(raw > 0, "generated HeerId must have a positive raw value");
    let round_tripped =
        heeranjid::HeerId::from_i64(raw).expect("generated i64 must parse as a HeerId");
    assert_eq!(id, round_tripped, "HeerId must round-trip through i64");

    // Node-id component must match what we asked for.
    assert_eq!(id.into_parts().node_id, 1, "node_id must be 1");

    teardown_schema(&client, schema_name).await;
}

// ---------------------------------------------------------------------------
// Single-row RanjId generation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_ranjid_returns_valid_id() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_generate_ranjid";
    setup_schema(&client, schema_name).await;

    let id = heeranjid::postgres_generate::generate_ranjid(&client, 1)
        .await
        .expect("generate_ranjid should succeed");

    // Round-trip: the returned RanjId must serialise to a UUID that
    // parses back to the same value via from_uuid.
    let uuid = id.as_uuid();
    let round_tripped =
        heeranjid::RanjId::from_uuid(uuid).expect("generated uuid must parse as a RanjId");
    assert_eq!(id, round_tripped, "RanjId must round-trip through uuid");

    // Node-id component must match what we asked for.
    assert_eq!(id.into_parts().node_id, 1, "node_id must be 1");

    teardown_schema(&client, schema_name).await;
}

// ---------------------------------------------------------------------------
// Batch HeerId generation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_heerids_returns_requested_count() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_generate_heerids";
    setup_schema(&client, schema_name).await;

    let count = 5_i32;
    let ids = heeranjid::postgres_generate::generate_heerids(&client, 1, count)
        .await
        .expect("generate_heerids should succeed");

    assert_eq!(
        ids.len(),
        count as usize,
        "batch generate_heerids must return exactly `count` rows"
    );

    for id in &ids {
        let raw = id.as_i64();
        assert!(raw > 0, "each generated HeerId must be positive");
        heeranjid::HeerId::from_i64(raw).expect("each raw i64 must parse as a HeerId");
        assert_eq!(id.into_parts().node_id, 1, "each id must belong to node 1");
    }

    // IDs must be strictly increasing (time-ordered, ascending).
    for pair in ids.windows(2) {
        assert!(
            pair[0] < pair[1],
            "batch-generated HeerIds must be strictly ascending"
        );
    }

    teardown_schema(&client, schema_name).await;
}

// ---------------------------------------------------------------------------
// Batch RanjId generation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_ranjids_returns_requested_count() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_generate_ranjids";
    setup_schema(&client, schema_name).await;

    let count = 5_i32;
    let ids = heeranjid::postgres_generate::generate_ranjids(&client, 1, count)
        .await
        .expect("generate_ranjids should succeed");

    assert_eq!(
        ids.len(),
        count as usize,
        "batch generate_ranjids must return exactly `count` rows"
    );

    for id in &ids {
        let uuid = id.as_uuid();
        heeranjid::RanjId::from_uuid(uuid).expect("each uuid must parse as a RanjId");
        assert_eq!(id.into_parts().node_id, 1, "each id must belong to node 1");
    }

    // IDs must be strictly increasing (time-ordered, ascending).
    for pair in ids.windows(2) {
        assert!(
            pair[0] < pair[1],
            "batch-generated RanjIds must be strictly ascending"
        );
    }

    teardown_schema(&client, schema_name).await;
}

// ---------------------------------------------------------------------------
// Error typing: rollback SQLSTATE variants
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_heerid_surfaces_logical_drift() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_logical_drift";
    setup_schema(&client, schema_name).await;

    // Set last_id_time to 1ms in the future (< 2ms threshold for logical drift)
    client
        .execute(
            "INSERT INTO heer_node_state (node_id, last_id_time, last_sequence) \
             SELECT 1, \
                    (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT \
                     - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 1, \
                    0",
            &[],
        )
        .await
        .expect("seed heer_node_state");

    let error = heeranjid::postgres_generate::generate_heerid(&client, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid::postgres_generate::GenerateError::LogicalDrift { .. } => {
            // Expected
        }
        _ => panic!("expected LogicalDrift, got {:?}", error),
    }

    teardown_schema(&client, schema_name).await;
}

#[tokio::test]
async fn generate_heerid_surfaces_clock_rollback() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_clock_rollback";
    setup_schema(&client, schema_name).await;

    // Set last_id_time to 10ms in the future (soft rollback band: 2-50ms)
    client
        .execute(
            "INSERT INTO heer_node_state (node_id, last_id_time, last_sequence) \
             SELECT 1, \
                    (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT \
                     - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 10, \
                    0",
            &[],
        )
        .await
        .expect("seed heer_node_state");

    let error = heeranjid::postgres_generate::generate_heerid(&client, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid::postgres_generate::GenerateError::ClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected ClockRollback, got {:?}", error),
    }

    teardown_schema(&client, schema_name).await;
}

#[tokio::test]
async fn generate_heerid_surfaces_hard_clock_rollback() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_hard_clock_rollback";
    setup_schema(&client, schema_name).await;

    // Set last_id_time to 100ms in the future (hard rollback band: >= 50ms)
    client
        .execute(
            "INSERT INTO heer_node_state (node_id, last_id_time, last_sequence) \
             SELECT 1, \
                    (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT \
                     - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 100, \
                    0",
            &[],
        )
        .await
        .expect("seed heer_node_state");

    let error = heeranjid::postgres_generate::generate_heerid(&client, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid::postgres_generate::GenerateError::HardClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected HardClockRollback, got {:?}", error),
    }

    teardown_schema(&client, schema_name).await;
}

#[tokio::test]
async fn generate_ranjid_surfaces_hard_clock_rollback() {
    let Some(client) = connect().await else {
        eprintln!("SKIP: DATABASE_URL not set; skipping live database test");
        return;
    };

    let schema_name = "test_heeranjid_ranj_hard_rollback";
    setup_schema(&client, schema_name).await;

    // Set last_id_time to a very large value to trigger hard clock rollback
    client
        .execute(
            "INSERT INTO heer_ranj_node_state (node_id, last_id_time, last_sequence) VALUES (1, 999999999999999, 0)",
            &[],
        )
        .await
        .expect("seed heer_ranj_node_state");

    let error = heeranjid::postgres_generate::generate_ranjid(&client, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid::postgres_generate::GenerateError::HardClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected HardClockRollback, got {:?}", error),
    }

    teardown_schema(&client, schema_name).await;
}
