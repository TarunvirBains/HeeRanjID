use heeranjid::{fetch_epoch, fetch_node, install_schema, validate_heer_node_id};
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

#[test]
fn validate_heer_node_id_rejects_out_of_range_values() {
    assert_eq!(validate_heer_node_id(0).unwrap(), 0);
    assert!(validate_heer_node_id(-1).is_err());
    assert!(validate_heer_node_id(512).is_err());
}
