//! M:N join-table asc→desc migration (spec §7.5).
//!
//! Exercises the three-table join-table playbook end-to-end against a
//! live Postgres: seed two parent tables and a join table whose two FK
//! columns reference each parent's PK, then migrate all three tables to
//! descending siblings under a **single mega-transaction** cutover
//! coordinating the entire graph.
//!
//! # Why dual-connect
//!
//! `heeranjid::postgres_schema::install_*` helpers take a
//! `tokio_postgres::GenericClient`, but the bulk SQL / fixtures run
//! through a `sqlx::PgPool`. We hold both connections side by side —
//! same pattern as `migrate_asc_to_desc_with_fk.rs`.
//!
//! # Join-table specifics (§7.5)
//!
//! The join table carries **two** `_desc` columns (one per FK). A
//! single trigger is installed on the join table with **two
//! `ColumnPair`s**, so one BEFORE INSERT/UPDATE firing keeps both
//! `user_id_desc` and `group_id_desc` in sync with their sources.
//!
//! The mega-cutover transaction drops: the join table's two FKs, then
//! the join table's composite PK, then each parent's PK. It promotes
//! each parent's `_desc` index into its new PK, adds a fresh composite
//! PK on the join table (`user_id_desc, group_id_desc`), and adds the
//! new join-table FKs as `NOT VALID` referencing the *still-named*
//! `id_desc` columns on each parent. Defaults are swapped, old
//! columns/triggers are dropped, and columns are renamed, all inside
//! the same `BEGIN ... COMMIT`. `VALIDATE CONSTRAINT` on the new FKs
//! happens after commit to keep the cutover window tight.

use heeranjid::postgres_schema::{
    ColumnPair, IdKind, drop_autofill_trigger_for_table, install_all_desc_support,
    install_autofill_trigger_for_table, install_schema, seed_default_node,
};
use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;

/// Spawn a dedicated `tokio_postgres::Client` alongside the sqlx pool.
async fn dual_connect(url: &str) -> tokio_postgres::Client {
    let (client, conn) = tokio_postgres::connect(url, tokio_postgres::NoTls)
        .await
        .expect("tokio-postgres connect");
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("tokio-postgres connection error: {e}");
        }
    });
    client
}

#[tokio::test(flavor = "multi_thread")]
async fn join_table_mn_migration_end_to_end() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect sqlx pool");
    let pg_client = dual_connect(&url).await;

    // --- Base schema + desc support (idempotent) ---
    install_schema(&pg_client).await.expect("install_schema");
    let _ = seed_default_node(&pg_client).await;
    install_all_desc_support(&pg_client)
        .await
        .expect("install_all_desc_support");

    // --- Fixture: drop any leftover (FK order: join first, then parents) ---
    pool.execute("DROP TABLE IF EXISTS jt_user_groups CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS jt_users CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS jt_groups CASCADE")
        .await
        .unwrap();

    pool.execute("CREATE TABLE jt_users (id bigint PRIMARY KEY DEFAULT heerid_next())")
        .await
        .unwrap();
    pool.execute("CREATE TABLE jt_groups (id bigint PRIMARY KEY DEFAULT heerid_next())")
        .await
        .unwrap();
    pool.execute(
        "CREATE TABLE jt_user_groups (
            user_id bigint NOT NULL REFERENCES jt_users(id),
            group_id bigint NOT NULL REFERENCES jt_groups(id),
            PRIMARY KEY (user_id, group_id)
         )",
    )
    .await
    .unwrap();

    // Pin a sqlx connection for fixture inserts (see
    // migrate_asc_to_desc_with_fk.rs for the rationale on pinning).
    let mut fixture_conn = pool.acquire().await.expect("acquire fixture conn");
    sqlx::query("SELECT set_heer_node_id(1)")
        .execute(&mut *fixture_conn)
        .await
        .expect("set_heer_node_id on fixture conn");

    // 50 users.
    sqlx::query("INSERT INTO jt_users SELECT heerid_next() FROM generate_series(1, 50)")
        .execute(&mut *fixture_conn)
        .await
        .unwrap();

    // 20 groups.
    sqlx::query("INSERT INTO jt_groups SELECT heerid_next() FROM generate_series(1, 20)")
        .execute(&mut *fixture_conn)
        .await
        .unwrap();

    // 200 join rows: a deterministic "user u is in group ((u + k) mod 20) + 1"
    // assignment for k ∈ [0, 4), yielding exactly 4 distinct groups per user
    // and 200 distinct (user, group) pairs total (50 * 4 = 200).
    //
    // We materialize parent IDs in order so the mapping is stable across
    // `heerid_next()` values — row_number gives us [1..=50] / [1..=20]
    // indexing independent of the actual stored ids.
    sqlx::query(
        "WITH
            u AS (
                SELECT id, row_number() OVER (ORDER BY id) - 1 AS rn
                FROM jt_users
            ),
            g AS (
                SELECT id, row_number() OVER (ORDER BY id) - 1 AS rn
                FROM jt_groups
            ),
            k AS (SELECT generate_series(0, 3) AS k)
         INSERT INTO jt_user_groups (user_id, group_id)
         SELECT u.id, g.id
         FROM u CROSS JOIN k
         JOIN g ON g.rn = ((u.rn + k.k) % 20)",
    )
    .execute(&mut *fixture_conn)
    .await
    .unwrap();
    drop(fixture_conn);

    let user_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let group_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    let ug_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_user_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(user_count, 50);
    assert_eq!(group_count, 20);
    assert_eq!(ug_count, 200);

    // --- Phase 1: preparation — add desc columns + install triggers ---
    pool.execute("ALTER TABLE jt_users ADD COLUMN id_desc bigint")
        .await
        .unwrap();
    pool.execute("ALTER TABLE jt_groups ADD COLUMN id_desc bigint")
        .await
        .unwrap();
    pool.execute(
        "ALTER TABLE jt_user_groups
            ADD COLUMN user_id_desc bigint,
            ADD COLUMN group_id_desc bigint",
    )
    .await
    .unwrap();

    install_autofill_trigger_for_table(
        &pg_client,
        "jt_users",
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install trigger on jt_users");

    install_autofill_trigger_for_table(
        &pg_client,
        "jt_groups",
        &[ColumnPair {
            src: "id",
            dst: "id_desc",
        }],
        IdKind::Heer,
    )
    .await
    .expect("install trigger on jt_groups");

    // Join table: a single trigger with TWO ColumnPairs (spec §7.5).
    install_autofill_trigger_for_table(
        &pg_client,
        "jt_user_groups",
        &[
            ColumnPair {
                src: "user_id",
                dst: "user_id_desc",
            },
            ColumnPair {
                src: "group_id",
                dst: "group_id_desc",
            },
        ],
        IdKind::Heer,
    )
    .await
    .expect("install trigger on jt_user_groups");

    // --- Phase 2: backfill (each call auto-commits) ---
    pool.execute("CALL heeranjid_bulk_backfill('jt_users','id','id_desc','heer',50)")
        .await
        .unwrap();
    pool.execute("CALL heeranjid_bulk_backfill('jt_groups','id','id_desc','heer',50)")
        .await
        .unwrap();
    pool.execute(
        "CALL heeranjid_bulk_backfill('jt_user_groups','user_id','user_id_desc','heer',100)",
    )
    .await
    .unwrap();
    pool.execute(
        "CALL heeranjid_bulk_backfill('jt_user_groups','group_id','group_id_desc','heer',100)",
    )
    .await
    .unwrap();

    // --- Phase 3: verification — stored = flip(src), NOT NULL form ---
    for (tbl, col) in [
        ("jt_users", "id_desc"),
        ("jt_groups", "id_desc"),
        ("jt_user_groups", "user_id_desc"),
        ("jt_user_groups", "group_id_desc"),
    ] {
        let missing: i64 =
            sqlx::query_scalar(format!("SELECT count(*) FROM {tbl} WHERE {col} IS NULL").as_str())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(missing, 0, "{tbl}.{col} fully backfilled");
    }

    // --- Phase 4: index builds (outside any transaction) ---
    pool.execute("CREATE UNIQUE INDEX CONCURRENTLY idx_jt_users_id_desc ON jt_users (id_desc)")
        .await
        .unwrap();
    pool.execute("CREATE UNIQUE INDEX CONCURRENTLY idx_jt_groups_id_desc ON jt_groups (id_desc)")
        .await
        .unwrap();
    pool.execute(
        "CREATE INDEX CONCURRENTLY idx_jt_ug_user_id_desc ON jt_user_groups (user_id_desc)",
    )
    .await
    .unwrap();
    pool.execute(
        "CREATE INDEX CONCURRENTLY idx_jt_ug_group_id_desc ON jt_user_groups (group_id_desc)",
    )
    .await
    .unwrap();

    // --- Phase 5: NOT NULL fast-path for each _desc column ---
    for (tbl, col, cname) in [
        ("jt_users", "id_desc", "jt_users_id_desc_nn"),
        ("jt_groups", "id_desc", "jt_groups_id_desc_nn"),
        ("jt_user_groups", "user_id_desc", "jt_ug_user_id_desc_nn"),
        ("jt_user_groups", "group_id_desc", "jt_ug_group_id_desc_nn"),
    ] {
        pool.execute(
            format!("ALTER TABLE {tbl} ADD CONSTRAINT {cname} CHECK ({col} IS NOT NULL) NOT VALID")
                .as_str(),
        )
        .await
        .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} VALIDATE CONSTRAINT {cname}").as_str())
            .await
            .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} ALTER COLUMN {col} SET NOT NULL").as_str())
            .await
            .unwrap();
        pool.execute(format!("ALTER TABLE {tbl} DROP CONSTRAINT {cname}").as_str())
            .await
            .unwrap();
    }

    // --- Phase 6: cutover — ONE atomic transaction across all three tables ---
    let mut tx = pool.begin().await.unwrap();

    // (1) Drop join-table FKs so parent PKs are free to move.
    sqlx::query("ALTER TABLE jt_user_groups DROP CONSTRAINT jt_user_groups_user_id_fkey")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_user_groups DROP CONSTRAINT jt_user_groups_group_id_fkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (2) Drop join-table composite PK.
    sqlx::query("ALTER TABLE jt_user_groups DROP CONSTRAINT jt_user_groups_pkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (3) Drop each parent's PK.
    sqlx::query("ALTER TABLE jt_users DROP CONSTRAINT jt_users_pkey")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_groups DROP CONSTRAINT jt_groups_pkey")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (4) Promote each parent's _desc index to PK.
    sqlx::query(
        "ALTER TABLE jt_users ADD CONSTRAINT jt_users_pkey PRIMARY KEY USING INDEX idx_jt_users_id_desc",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE jt_groups ADD CONSTRAINT jt_groups_pkey PRIMARY KEY USING INDEX idx_jt_groups_id_desc",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (5) New composite PK on the join table. Postgres will build a
    //     fresh composite index here; the old composite PK index is
    //     already gone, and the per-column desc indexes from phase 4
    //     are non-unique so they can't be promoted directly.
    sqlx::query(
        "ALTER TABLE jt_user_groups
            ADD CONSTRAINT jt_user_groups_pkey PRIMARY KEY (user_id_desc, group_id_desc)",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (6) New join-table FKs as NOT VALID, referencing the
    //     still-named `id_desc` columns on each parent.
    sqlx::query(
        "ALTER TABLE jt_user_groups
            ADD CONSTRAINT jt_ug_user_fk FOREIGN KEY (user_id_desc)
            REFERENCES jt_users(id_desc) NOT VALID",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE jt_user_groups
            ADD CONSTRAINT jt_ug_group_fk FOREIGN KEY (group_id_desc)
            REFERENCES jt_groups(id_desc) NOT VALID",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    // (7) Swap defaults on parents.
    sqlx::query("ALTER TABLE jt_users ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_users ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_groups ALTER COLUMN id_desc SET DEFAULT heerid_next_desc()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_groups ALTER COLUMN id DROP DEFAULT")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (8) Drop old asc columns on all three tables.
    sqlx::query("ALTER TABLE jt_users DROP COLUMN id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_groups DROP COLUMN id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_user_groups DROP COLUMN user_id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_user_groups DROP COLUMN group_id")
        .execute(&mut *tx)
        .await
        .unwrap();

    // (9) Drop triggers + per-table trigger functions on all three tables.
    for tbl in ["jt_users", "jt_groups", "jt_user_groups"] {
        sqlx::query(format!("DROP TRIGGER IF EXISTS zzz_{tbl}_autofill_desc ON {tbl}").as_str())
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(format!("DROP FUNCTION IF EXISTS zzz_{tbl}_autofill_desc() CASCADE").as_str())
            .execute(&mut *tx)
            .await
            .unwrap();
    }

    // (10) Rename columns into place.
    sqlx::query("ALTER TABLE jt_users RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_groups RENAME COLUMN id_desc TO id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_user_groups RENAME COLUMN user_id_desc TO user_id")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE jt_user_groups RENAME COLUMN group_id_desc TO group_id")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // --- Phase 7: validate the deferred FKs outside the cutover tx ---
    pool.execute("ALTER TABLE jt_user_groups VALIDATE CONSTRAINT jt_ug_user_fk")
        .await
        .unwrap();
    pool.execute("ALTER TABLE jt_user_groups VALIDATE CONSTRAINT jt_ug_group_fk")
        .await
        .unwrap();

    // --- Assertions ---

    // (a) Row counts preserved.
    let post_user_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let post_group_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    let post_ug_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jt_user_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(post_user_count, 50);
    assert_eq!(post_group_count, 20);
    assert_eq!(post_ug_count, 200);

    // (b) FK integrity both directions.
    let orphan_users: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jt_user_groups ug
         LEFT JOIN jt_users u ON ug.user_id = u.id
         WHERE u.id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphan_users, 0, "no orphaned user_ids in jt_user_groups");

    let orphan_groups: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jt_user_groups ug
         LEFT JOIN jt_groups g ON ug.group_id = g.id
         WHERE g.id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphan_groups, 0, "no orphaned group_ids in jt_user_groups");

    // (c) Descending sort: plain ORDER BY id on jt_users returns the
    //     3 most recent rows in reverse-chronological order.
    let user_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM jt_users ORDER BY id LIMIT 3")
        .fetch_all(&pool)
        .await
        .unwrap();
    let user_ts: Vec<u64> = user_ids
        .iter()
        .map(|&raw| heeranjid::HeerIdDesc::from_i64(raw).unwrap().timestamp_ms())
        .collect();
    assert!(
        user_ts.windows(2).all(|w| w[0] >= w[1]),
        "jt_users ORDER BY id returns reverse-chronological rows: {user_ts:?}"
    );

    // (d) Old columns gone on all three tables.
    let user_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'jt_users' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(user_cols, vec!["id".to_string()]);

    let group_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'jt_groups' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(group_cols, vec!["id".to_string()]);

    let ug_cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'jt_user_groups' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ug_cols, vec!["group_id".to_string(), "user_id".to_string()]);

    // --- Cleanup ---
    let _ = drop_autofill_trigger_for_table(&pg_client, "jt_users").await;
    let _ = drop_autofill_trigger_for_table(&pg_client, "jt_groups").await;
    let _ = drop_autofill_trigger_for_table(&pg_client, "jt_user_groups").await;
    pool.execute("DROP TABLE IF EXISTS jt_user_groups CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS jt_users CASCADE")
        .await
        .unwrap();
    pool.execute("DROP TABLE IF EXISTS jt_groups CASCADE")
        .await
        .unwrap();
}
