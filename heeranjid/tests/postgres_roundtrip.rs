//! tokio-postgres roundtrip tests for the `postgres` codec feature.
//!
//! Each test opens a real connection, creates a temporary table, inserts a
//! value using the `postgres_types::ToSql` impl, reads it back with the
//! corresponding `FromSql` impl, and asserts bit-exact equality.
//!
//! Requires a running Postgres instance reachable via the `DATABASE_URL`
//! environment variable. The test suite uses `tokio::test` for async
//! execution and `tokio-postgres` with `NoTls` for the connection.
//!
//! Tests are compiled only when the `postgres` feature is enabled.

#![cfg(feature = "postgres")]

use std::env;
use tokio_postgres::NoTls;

async fn connect() -> tokio_postgres::Client {
    let url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://djogi:djogi@localhost:5432/djogi_test".to_owned());
    let (client, conn) = tokio_postgres::connect(&url, NoTls)
        .await
        .expect("failed to connect to Postgres");
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("postgres connection error: {e}");
        }
    });
    client
}

// ---------------------------------------------------------------------------
// HeerId — BIGINT roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn heerid_roundtrip_through_bigint_column() {
    use heeranjid::HeerId;

    let client = connect().await;

    client
        .execute("CREATE TEMP TABLE heerid_rt (val BIGINT NOT NULL)", &[])
        .await
        .expect("create temp table");

    let original = HeerId::new(1_234_567, 42, 777).unwrap();

    client
        .execute("INSERT INTO heerid_rt (val) VALUES ($1)", &[&original])
        .await
        .expect("insert HeerId");

    let row = client
        .query_one("SELECT val FROM heerid_rt", &[])
        .await
        .expect("select HeerId");

    let retrieved: HeerId = row.get(0);
    assert_eq!(original, retrieved, "HeerId did not roundtrip correctly");
}

// ---------------------------------------------------------------------------
// RanjId — UUID roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ranjid_roundtrip_through_uuid_column() {
    use heeranjid::{RanjId, RanjPrecision};

    let client = connect().await;

    client
        .execute("CREATE TEMP TABLE ranjid_rt (val UUID NOT NULL)", &[])
        .await
        .expect("create temp table");

    let original =
        RanjId::new(9_876_543_210_u128, RanjPrecision::Microseconds, 511, 65535).unwrap();

    client
        .execute("INSERT INTO ranjid_rt (val) VALUES ($1)", &[&original])
        .await
        .expect("insert RanjId");

    let row = client
        .query_one("SELECT val FROM ranjid_rt", &[])
        .await
        .expect("select RanjId");

    let retrieved: RanjId = row.get(0);
    assert_eq!(original, retrieved, "RanjId did not roundtrip correctly");
}

// ---------------------------------------------------------------------------
// RanjId — rejects non-UUIDv8 stored in a UUID column
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ranjid_rejects_non_v8_uuid() {
    use uuid::Uuid;

    let client = connect().await;

    client
        .execute("CREATE TEMP TABLE ranjid_bad (val UUID NOT NULL)", &[])
        .await
        .expect("create temp table");

    // Construct a UUIDv4 by hand: version nibble at bits 76-79 = 0x4,
    // variant at 62-63 = 0b10 (RFC 4122 standard).
    let raw: u128 = (0x4u128 << 76) | (0x2u128 << 62) | 0xABCD_EF01_2345u128;
    let v4 = Uuid::from_u128(raw);
    assert_eq!(v4.get_version_num(), 4, "sanity: test UUID must be v4");

    // Insert the raw UUID (Uuid implements ToSql directly).
    client
        .execute("INSERT INTO ranjid_bad (val) VALUES ($1)", &[&v4])
        .await
        .expect("insert raw v4 uuid");

    // Attempt to decode it as RanjId — this must fail.
    let row = client
        .query_one("SELECT val FROM ranjid_bad", &[])
        .await
        .expect("select row");

    let result: Result<heeranjid::RanjId, _> = row.try_get(0);
    assert!(
        result.is_err(),
        "expected FromSql decode to fail for a non-UUIDv8 value, but it succeeded"
    );
}
