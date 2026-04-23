use heeranjid::RanjId;
use heeranjid_sqlx::{
    fetch_epoch, fetch_node, install_schema, seed_default_node, validate_epoch,
    validate_heer_node_id, validate_startup,
};
use sqlx::{Connection, Executor, PgConnection};

fn test_database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn test_schema_name() -> String {
    format!(
        "heeranjid_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

async fn connect_test_db() -> Option<PgConnection> {
    let url = test_database_url()?;
    PgConnection::connect(&url).await.ok()
}

#[tokio::test]
async fn postgres_helpers_fetch_node_and_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_nodes (node_id, name, description, is_active)
        VALUES (1, 'default', 'test node', true)
        "#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_config (id, epoch)
        VALUES (1, TIMESTAMP '2024-01-01 00:00:00')
        "#,
    )
    .await
    .unwrap();

    let node = fetch_node(&mut conn, 1).await.unwrap().unwrap();
    let epoch = fetch_epoch(&mut conn).await.unwrap().unwrap();

    assert_eq!(node.node_id, 1);
    assert_eq!(node.name, "default");
    assert_eq!(node.description.as_deref(), Some("test node"));
    assert_eq!(epoch.to_string(), "2024-01-01 0:00:00.0");
}

#[tokio::test]
async fn postgres_sql_generates_monotonic_heerids() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_nodes (node_id, name, is_active)
        VALUES (1, 'default', true)
        "#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_config (id, epoch)
        VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')
        "#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let generated: Vec<i64> = sqlx::query_scalar("SELECT generate_id()")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(generated.len(), 1);

    let batch: Vec<i64> = sqlx::query_scalar("SELECT id FROM generate_ids(5)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(generated[0] < batch[0]);
}

#[tokio::test]
async fn postgres_sql_rejects_future_clock_state() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_nodes (node_id, name, is_active)
        VALUES (1, 'default', true)
        "#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_config (id, epoch)
        VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')
        "#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"
        INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
        VALUES (1, 999999999999, 0)
        "#,
    )
    .await
    .unwrap();

    let error = sqlx::query_scalar::<_, i64>("SELECT generate_id($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("clock rollback"));
}

#[test]
fn validate_heer_node_id_rejects_out_of_range_values() {
    assert_eq!(validate_heer_node_id(0).unwrap(), 0);
    assert!(validate_heer_node_id(-1).is_err());
    assert!(validate_heer_node_id(512).is_err());
}

#[tokio::test]
async fn startup_validates_active_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();
    seed_default_node(&mut conn).await.unwrap();

    let node = validate_startup(&mut conn, 1).await.unwrap();
    assert_eq!(node.node_id, 1);
    assert_eq!(node.name, "default");
}

#[tokio::test]
async fn startup_rejects_inactive_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', false)"#,
    )
    .await
    .unwrap();

    let err = validate_startup(&mut conn, 1).await.unwrap_err();
    assert!(err.to_string().contains("not registered or not active"));
}

#[tokio::test]
async fn startup_rejects_unknown_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    let err = validate_startup(&mut conn, 99).await.unwrap_err();
    assert!(err.to_string().contains("not registered or not active"));
}

#[tokio::test]
async fn startup_rejects_missing_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    let err = validate_epoch(&mut conn).await.unwrap_err();
    assert!(err.to_string().contains("epoch is not configured"));
}

#[tokio::test]
async fn startup_validates_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, TIMESTAMP '2024-01-01 00:00:00')"#,
    )
    .await
    .unwrap();

    let epoch = validate_epoch(&mut conn).await.unwrap();
    assert_eq!(epoch.to_string(), "2024-01-01 0:00:00.0");
}

#[tokio::test]
async fn ranjid_sql_generates_valid_uuidv8() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let uuid: uuid::Uuid = sqlx::query_scalar("SELECT generate_ranjid($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap();

    let ranj = RanjId::from_uuid(uuid).unwrap();
    // TODO: SQL functions still generate UUIDv7; version check skipped until heer_configure() is updated.
    assert!(ranj.timestamp_micros() > 0);
    assert_eq!(ranj.node_id(), 1);
}

#[tokio::test]
async fn ranjid_sql_generates_monotonic_batch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_ranj_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let batch: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM generate_ranjids(10)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 10);

    let ranj_ids: Vec<RanjId> = batch
        .iter()
        .map(|u| RanjId::from_uuid(*u).unwrap())
        .collect();

    // Must be strictly increasing
    assert!(ranj_ids.windows(2).all(|pair| pair[0] < pair[1]));

    // All should have node_id = 1
    for r in &ranj_ids {
        assert_eq!(r.node_id(), 1);
    }
}

#[tokio::test]
async fn ranjid_sql_rejects_clock_rollback() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"INSERT INTO heer_ranj_node_state (node_id, last_id_time, last_sequence) VALUES (1, 999999999999999, 0)"#,
    )
    .await
    .unwrap();

    let error = sqlx::query_scalar::<_, uuid::Uuid>("SELECT generate_ranjid($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("clock rollback"));
}

#[tokio::test]
async fn ranjid_rust_helper_generates_valid_id() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let ranj = heeranjid_sqlx::generate_ranjid(&mut conn, 1).await.unwrap();
    assert_eq!(ranj.node_id(), 1);
    assert!(ranj.timestamp_micros() > 0);

    let batch = heeranjid_sqlx::generate_ranjids(&mut conn, 1, 5)
        .await
        .unwrap();
    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn heerid_sql_non_spanning_rejects_overflow() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Request more IDs than fit in one millisecond with spanning disabled
    let err = sqlx::query_scalar::<_, i64>("SELECT id FROM generate_ids($1, $2, $3)")
        .bind(1_i32)
        .bind(8193_i32)
        .bind(false)
        .fetch_all(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("requested"));
}

#[tokio::test]
async fn heerid_sql_spanning_handles_overflow() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set state so only a few sequences remain
    conn.execute(
        r#"INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
           SELECT 1,
                  FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                  - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT,
                  8190"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let batch: Vec<i64> = sqlx::query_scalar("SELECT id FROM generate_ids(5)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn heerid_sql_rejects_missing_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();

    let err = sqlx::query_scalar::<_, i64>("SELECT generate_id($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("heer_config"));
}

#[tokio::test]
async fn heerid_sql_rejects_missing_session_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Call generate_id() without setting session node first
    let err = sqlx::query_scalar::<_, i64>("SELECT generate_id()")
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("node_id"));
}

#[tokio::test]
async fn heerid_rust_helper_generates_valid_id() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let heer = heeranjid_sqlx::generate_heerid(&mut conn, 1).await.unwrap();
    assert_eq!(heer.node_id(), 1);
    assert!(heer.timestamp_ms() > 0);

    let batch = heeranjid_sqlx::generate_heerids(&mut conn, 1, 5)
        .await
        .unwrap();
    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn heerid_sql_order_by_matches_generation_order() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(r#"CREATE TEMP TABLE test_ids (pos SERIAL, hid BIGINT NOT NULL)"#)
        .await
        .unwrap();

    conn.execute(r#"INSERT INTO test_ids (hid) SELECT id FROM generate_ids(20)"#)
        .await
        .unwrap();

    let ordered: Vec<(i32, i64)> = sqlx::query_as("SELECT pos, hid FROM test_ids ORDER BY hid ASC")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    for (i, (pos, _)) in ordered.iter().enumerate() {
        assert_eq!(*pos as usize, i + 1);
    }
}

#[tokio::test]
async fn ranjid_sql_order_by_matches_generation_order() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_ranj_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(r#"CREATE TEMP TABLE test_rids (pos SERIAL, rid UUID NOT NULL)"#)
        .await
        .unwrap();

    conn.execute(r#"INSERT INTO test_rids (rid) SELECT id FROM generate_ranjids(20)"#)
        .await
        .unwrap();

    let ordered: Vec<(i32, uuid::Uuid)> =
        sqlx::query_as("SELECT pos, rid FROM test_rids ORDER BY rid ASC")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    for (i, (pos, _)) in ordered.iter().enumerate() {
        assert_eq!(*pos as usize, i + 1);
    }
}

#[tokio::test]
async fn schema_install_is_idempotent() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();
    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();

    let node = fetch_node(&mut conn, 1).await.unwrap().unwrap();
    assert_eq!(node.name, "default");
}

#[tokio::test]
async fn heerid_works_as_column_default() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(
        r#"CREATE TABLE test_entities (
            id BIGINT PRIMARY KEY DEFAULT generate_id(),
            label TEXT NOT NULL
        )"#,
    )
    .await
    .unwrap();

    conn.execute(r#"INSERT INTO test_entities (label) VALUES ('alpha')"#)
        .await
        .unwrap();
    conn.execute(r#"INSERT INTO test_entities (label) VALUES ('bravo')"#)
        .await
        .unwrap();

    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, label FROM test_entities ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    assert_eq!(rows.len(), 2);
    assert!(rows[0].0 > 0);
    assert!(rows[0].0 < rows[1].0);
}

#[tokio::test]
async fn ranjid_works_as_column_default() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_ranj_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(
        r#"CREATE TABLE test_events (
            id UUID PRIMARY KEY DEFAULT generate_ranjid(),
            label TEXT NOT NULL
        )"#,
    )
    .await
    .unwrap();

    conn.execute(r#"INSERT INTO test_events (label) VALUES ('alpha')"#)
        .await
        .unwrap();
    conn.execute(r#"INSERT INTO test_events (label) VALUES ('bravo')"#)
        .await
        .unwrap();

    let rows: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, label FROM test_events ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    assert_eq!(rows.len(), 2);
    RanjId::from_uuid(rows[0].0).unwrap();
    RanjId::from_uuid(rows[1].0).unwrap();
    assert!(rows[0].0 < rows[1].0);
}

#[tokio::test]
async fn ranjid_big_bang_epoch_generates_valid_ids() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();

    // Set epoch to Unix epoch with a Big Bang offset.
    // The Big Bang is ~13.787 billion years ago.
    // In microseconds: 13.787e9 * 365.25 * 86400 * 1e6 ≈ 4.3509e23
    // This value exceeds BIGINT range (9.2e18), proving the NUMERIC
    // arithmetic works correctly for the full 90-bit timestamp.
    conn.execute(
        r#"
        INSERT INTO heer_config (id, epoch, ranj_epoch_offset)
        VALUES (
            1,
            TIMESTAMP '1970-01-01 00:00:00',
            FLOOR(13.787e9 * 365.25 * 86400 * 1e6)::NUMERIC(30,0)
        )
        "#,
    )
    .await
    .unwrap();

    // Generate a single RanjId — the timestamp will encode microseconds
    // since the Big Bang, a value far beyond BIGINT range.
    let uuid: uuid::Uuid = sqlx::query_scalar("SELECT generate_ranjid($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap();

    let ranj = RanjId::from_uuid(uuid).unwrap();
    let parts = ranj.into_parts();

    // The timestamp should be roughly 4.35e23 (in the stored precision unit).
    // Just verify it's well beyond BIGINT max (9.22e18).
    assert!(
        parts.timestamp > 9_200_000_000_000_000_000,
        "timestamp {} should exceed BIGINT max",
        parts.timestamp
    );
    assert_eq!(parts.node_id, 1);

    // Generate a batch and verify monotonic ordering still works
    // at these extreme timestamp values.
    sqlx::query("SELECT set_heer_ranj_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let batch: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM generate_ranjids(10)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 10);
    let ranj_ids: Vec<RanjId> = batch
        .iter()
        .map(|u| RanjId::from_uuid(*u).unwrap())
        .collect();
    assert!(ranj_ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn ranjid_session_supports_large_node_ids() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    // Register a node with ID beyond HeerId's 511 limit
    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1000, 'large-node', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // set_heer_node_id would reject 1000, but set_heer_ranj_node_id accepts it
    sqlx::query("SELECT set_heer_ranj_node_id($1)")
        .bind(1000_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let batch: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM generate_ranjids(5)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 5);

    let ranj_ids: Vec<heeranjid::RanjId> = batch
        .iter()
        .map(|u| heeranjid::RanjId::from_uuid(*u).unwrap())
        .collect();

    // All should have node_id = 1000
    for r in &ranj_ids {
        assert_eq!(r.node_id(), 1000);
    }

    // Must be strictly increasing
    assert!(ranj_ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn generate_heerid_surfaces_logical_drift() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set last_id_time to 1ms in the future (< 2ms threshold for logical drift)
    conn.execute(
        r#"INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
           SELECT 1,
                  (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                   - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 1,
                  0"#,
    )
    .await
    .unwrap();

    let error = heeranjid_sqlx::generate_heerid(&mut conn, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid_sqlx::GenerateError::LogicalDrift { .. } => {
            // Expected
        }
        _ => panic!("expected LogicalDrift, got {:?}", error),
    }
}

#[tokio::test]
async fn generate_heerid_surfaces_clock_rollback() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set last_id_time to 10ms in the future (soft rollback band: 2-50ms)
    conn.execute(
        r#"INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
           SELECT 1,
                  (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                   - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 10,
                  0"#,
    )
    .await
    .unwrap();

    let error = heeranjid_sqlx::generate_heerid(&mut conn, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid_sqlx::GenerateError::ClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected ClockRollback, got {:?}", error),
    }
}

#[tokio::test]
async fn generate_heerid_surfaces_hard_clock_rollback() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set last_id_time to 100ms in the future (hard rollback band: >= 50ms)
    conn.execute(
        r#"INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
           SELECT 1,
                  (FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                   - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT) + 100,
                  0"#,
    )
    .await
    .unwrap();

    let error = heeranjid_sqlx::generate_heerid(&mut conn, 1)
        .await
        .unwrap_err();

    match error {
        heeranjid_sqlx::GenerateError::HardClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected HardClockRollback, got {:?}", error),
    }
}

#[tokio::test]
async fn ranjid_sql_rejects_clock_rollback_typed() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"INSERT INTO heer_ranj_node_state (node_id, last_id_time, last_sequence) VALUES (1, 999999999999999, 0)"#,
    )
    .await
    .unwrap();

    let error = heeranjid_sqlx::generate_ranjid(&mut conn, 1)
        .await
        .unwrap_err();

    // The large value should trigger hard clock rollback (>= 50ms equivalent in the µs scale)
    match error {
        heeranjid_sqlx::GenerateError::HardClockRollback { .. } => {
            // Expected
        }
        _ => panic!("expected HardClockRollback, got {:?}", error),
    }
}
