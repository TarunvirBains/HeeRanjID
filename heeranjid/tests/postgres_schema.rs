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
