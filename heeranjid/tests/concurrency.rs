use heeranjid::{HeerId, RanjId};
use sqlx::{Executor, PgPool};
use std::collections::HashSet;
use tokio::task::JoinSet;

const TASKS_PER_NODE: usize = 20;
const BATCH_SIZE: i32 = 100;
const MULTI_NODE_TASKS: usize = 20;
const MULTI_NODE_BATCH: i32 = 10;
const MAX_RETRIES: usize = 50;

fn test_database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn test_schema_name() -> String {
    format!(
        "heeranjid_conc_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

async fn setup_pool(schema: &str) -> Option<PgPool> {
    let url = test_database_url()?;
    // Keep pool small: 6 tests run in parallel, Postgres default max is 100.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(15)
        .connect(&url)
        .await
        .ok()?;

    pool.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();

    let install_sql = format!(
        r#"SET search_path TO "{schema}"; {}"#,
        heeranjid::INSTALL_SQL
    );
    sqlx::raw_sql(&install_sql).execute(&pool).await.unwrap();

    Some(pool)
}

async fn pooled_conn(pool: &PgPool, schema: &str) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut conn = pool.acquire().await.unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();
    conn
}

async fn seed_node(pool: &PgPool, schema: &str, node_id: i32) {
    let mut conn = pooled_conn(pool, schema).await;
    conn.execute(
        sqlx::query("INSERT INTO heer_nodes (node_id, name, is_active) VALUES ($1, $2, true)")
            .bind(node_id)
            .bind(format!("node-{node_id}")),
    )
    .await
    .unwrap();
}

async fn seed_epoch(pool: &PgPool, schema: &str) {
    let mut conn = pooled_conn(pool, schema).await;
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();
}

/// Returns true if the error is a minor clock rollback (retryable).
fn is_clock_rollback(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db_err) => db_err.message().contains("clock rollback"),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Parallel single HeerId generation on one node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_single_heerid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    seed_node(&pool, &schema, 1).await;
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for _ in 0..TASKS_PER_NODE {
        let pool = pool.clone();
        let schema = schema.clone();
        set.spawn(async move {
            for _ in 0..MAX_RETRIES {
                let mut conn = pooled_conn(&pool, &schema).await;
                match sqlx::query_scalar::<_, i64>("SELECT generate_id($1)")
                    .bind(1_i32)
                    .fetch_one(conn.as_mut())
                    .await
                {
                    Ok(raw) => return raw,
                    Err(e) if is_clock_rollback(&e) => continue,
                    Err(e) => panic!("unexpected error: {e}"),
                }
            }
            panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
        });
    }

    let mut ids = Vec::with_capacity(TASKS_PER_NODE);
    while let Some(result) = set.join_next().await {
        ids.push(result.unwrap());
    }

    let unique: HashSet<i64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), TASKS_PER_NODE, "expected all unique HeerIds");

    for raw in &ids {
        let heer = HeerId::from_i64(*raw).unwrap();
        assert_eq!(heer.node_id(), 1);
    }
}

// ---------------------------------------------------------------------------
// Test 2: Parallel batch HeerId generation on one node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_batch_heerid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    seed_node(&pool, &schema, 1).await;
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for _ in 0..TASKS_PER_NODE {
        let pool = pool.clone();
        let schema = schema.clone();
        set.spawn(async move {
            for _ in 0..MAX_RETRIES {
                let mut conn = pooled_conn(&pool, &schema).await;
                match sqlx::query_scalar::<_, i64>("SELECT id FROM generate_ids($1, $2)")
                    .bind(1_i32)
                    .bind(BATCH_SIZE)
                    .fetch_all(conn.as_mut())
                    .await
                {
                    Ok(rows) => return rows,
                    Err(e) if is_clock_rollback(&e) => continue,
                    Err(e) => panic!("unexpected error: {e}"),
                }
            }
            panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
        });
    }

    let mut all_ids = Vec::with_capacity(TASKS_PER_NODE * BATCH_SIZE as usize);
    while let Some(result) = set.join_next().await {
        let batch = result.unwrap();
        assert_eq!(batch.len(), BATCH_SIZE as usize);
        assert!(batch.windows(2).all(|w| w[0] < w[1]));
        all_ids.extend(batch);
    }

    let unique: HashSet<i64> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        TASKS_PER_NODE * BATCH_SIZE as usize,
        "expected all unique HeerIds across batches"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Parallel single RanjId generation on one node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_single_ranjid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    seed_node(&pool, &schema, 1).await;
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for _ in 0..TASKS_PER_NODE {
        let pool = pool.clone();
        let schema = schema.clone();
        set.spawn(async move {
            for _ in 0..MAX_RETRIES {
                let mut conn = pooled_conn(&pool, &schema).await;
                match sqlx::query_scalar::<_, uuid::Uuid>("SELECT generate_ranjid($1)")
                    .bind(1_i32)
                    .fetch_one(conn.as_mut())
                    .await
                {
                    Ok(uuid) => return uuid,
                    Err(e) if is_clock_rollback(&e) => continue,
                    Err(e) => panic!("unexpected error: {e}"),
                }
            }
            panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
        });
    }

    let mut ids = Vec::with_capacity(TASKS_PER_NODE);
    while let Some(result) = set.join_next().await {
        ids.push(result.unwrap());
    }

    let unique: HashSet<uuid::Uuid> = ids.iter().copied().collect();
    assert_eq!(unique.len(), TASKS_PER_NODE, "expected all unique RanjIds");

    for uuid in &ids {
        let ranj = RanjId::from_uuid(*uuid).unwrap();
        assert_eq!(ranj.node_id(), 1);
    }
}

// ---------------------------------------------------------------------------
// Test 4: Parallel batch RanjId generation on one node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_batch_ranjid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    seed_node(&pool, &schema, 1).await;
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for _ in 0..TASKS_PER_NODE {
        let pool = pool.clone();
        let schema = schema.clone();
        set.spawn(async move {
            for _ in 0..MAX_RETRIES {
                let mut conn = pooled_conn(&pool, &schema).await;
                match sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM generate_ranjids($1, $2)")
                    .bind(1_i32)
                    .bind(BATCH_SIZE)
                    .fetch_all(conn.as_mut())
                    .await
                {
                    Ok(rows) => return rows,
                    Err(e) if is_clock_rollback(&e) => continue,
                    Err(e) => panic!("unexpected error: {e}"),
                }
            }
            panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
        });
    }

    let mut all_ids = Vec::with_capacity(TASKS_PER_NODE * BATCH_SIZE as usize);
    while let Some(result) = set.join_next().await {
        let batch = result.unwrap();
        assert_eq!(batch.len(), BATCH_SIZE as usize);
        let ranj_batch: Vec<RanjId> = batch
            .iter()
            .map(|u| RanjId::from_uuid(*u).unwrap())
            .collect();
        assert!(ranj_batch.windows(2).all(|w| w[0] < w[1]));
        all_ids.extend(batch);
    }

    let unique: HashSet<uuid::Uuid> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        TASKS_PER_NODE * BATCH_SIZE as usize,
        "expected all unique RanjIds across batches"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Multi-node contention (3 nodes, 20 tasks each, 10 IDs per task)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_multi_node_heerid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    let node_ids: Vec<i32> = vec![1, 2, 3];
    for &nid in &node_ids {
        seed_node(&pool, &schema, nid).await;
    }
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for &node_id in &node_ids {
        for _ in 0..MULTI_NODE_TASKS {
            let pool = pool.clone();
            let schema = schema.clone();
            set.spawn(async move {
                for _ in 0..MAX_RETRIES {
                    let mut conn = pooled_conn(&pool, &schema).await;
                    match sqlx::query_scalar::<_, i64>("SELECT id FROM generate_ids($1, $2)")
                        .bind(node_id)
                        .bind(MULTI_NODE_BATCH)
                        .fetch_all(conn.as_mut())
                        .await
                    {
                        Ok(rows) => return (node_id, rows),
                        Err(e) if is_clock_rollback(&e) => continue,
                        Err(e) => panic!("unexpected error: {e}"),
                    }
                }
                panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
            });
        }
    }

    let mut all_ids: Vec<i64> = Vec::new();
    let mut per_node: std::collections::HashMap<i32, Vec<i64>> = std::collections::HashMap::new();

    while let Some(result) = set.join_next().await {
        let (node_id, batch) = result.unwrap();
        assert_eq!(batch.len(), MULTI_NODE_BATCH as usize);
        assert!(batch.windows(2).all(|w| w[0] < w[1]));
        for raw in &batch {
            let heer = HeerId::from_i64(*raw).unwrap();
            assert_eq!(heer.node_id(), node_id as u16);
        }
        all_ids.extend(&batch);
        per_node.entry(node_id).or_default().extend(batch);
    }

    let expected_total = node_ids.len() * MULTI_NODE_TASKS * MULTI_NODE_BATCH as usize;
    let unique: HashSet<i64> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        expected_total,
        "all IDs must be globally unique"
    );

    for &nid in &node_ids {
        assert_eq!(
            per_node[&nid].len(),
            MULTI_NODE_TASKS * MULTI_NODE_BATCH as usize
        );
    }
}

#[tokio::test]
async fn concurrent_multi_node_ranjid_generation() {
    let schema = test_schema_name();
    let pool = match setup_pool(&schema).await {
        Some(p) => p,
        None => return,
    };

    let node_ids: Vec<i32> = vec![1, 2, 3];
    for &nid in &node_ids {
        seed_node(&pool, &schema, nid).await;
    }
    seed_epoch(&pool, &schema).await;

    let mut set = JoinSet::new();
    for &node_id in &node_ids {
        for _ in 0..MULTI_NODE_TASKS {
            let pool = pool.clone();
            let schema = schema.clone();
            set.spawn(async move {
                for _ in 0..MAX_RETRIES {
                    let mut conn = pooled_conn(&pool, &schema).await;
                    match sqlx::query_scalar::<_, uuid::Uuid>(
                        "SELECT id FROM generate_ranjids($1, $2)",
                    )
                    .bind(node_id)
                    .bind(MULTI_NODE_BATCH)
                    .fetch_all(conn.as_mut())
                    .await
                    {
                        Ok(rows) => return (node_id, rows),
                        Err(e) if is_clock_rollback(&e) => continue,
                        Err(e) => panic!("unexpected error: {e}"),
                    }
                }
                panic!("exceeded {MAX_RETRIES} retries due to clock rollback");
            });
        }
    }

    let mut all_ids: Vec<uuid::Uuid> = Vec::new();
    let mut per_node: std::collections::HashMap<i32, Vec<uuid::Uuid>> =
        std::collections::HashMap::new();

    while let Some(result) = set.join_next().await {
        let (node_id, batch) = result.unwrap();
        assert_eq!(batch.len(), MULTI_NODE_BATCH as usize);
        let ranj_batch: Vec<RanjId> = batch
            .iter()
            .map(|u| RanjId::from_uuid(*u).unwrap())
            .collect();
        assert!(ranj_batch.windows(2).all(|w| w[0] < w[1]));
        for r in &ranj_batch {
            assert_eq!(r.node_id(), node_id as u16);
        }
        all_ids.extend(&batch);
        per_node.entry(node_id).or_default().extend(batch);
    }

    let expected_total = node_ids.len() * MULTI_NODE_TASKS * MULTI_NODE_BATCH as usize;
    let unique: HashSet<uuid::Uuid> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        expected_total,
        "all RanjIds must be globally unique"
    );

    for &nid in &node_ids {
        assert_eq!(
            per_node[&nid].len(),
            MULTI_NODE_TASKS * MULTI_NODE_BATCH as usize
        );
    }
}
