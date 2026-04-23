"""Runnable asc → desc migration example against MSSQL.

Demonstrates the full v0.3.1 HeeRanjIdDirectionFlip flow end-to-end:
install v0.2.x schema + v0.3.1 desc surface, create a sample table,
seed rows, apply the flip, verify reverse-chronological ordering by
the PK.

Requires MSSQL_URL environment variable:
  export MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;TrustServerCertificate=yes'
  python bindings/python/django/examples/migrate_asc_to_desc_mssql.py
"""
import os
import sys
import time
import uuid

MSSQL_URL = os.environ.get("MSSQL_URL")
if not MSSQL_URL:
    sys.exit(
        "MSSQL_URL not set; start 'docker compose up mssql -d' "
        "and export the ODBC connection string."
    )

try:
    import pyodbc
except ImportError:
    sys.exit("pyodbc not installed; run 'pip install pyodbc'")

from heeranjid import HeerIdDesc, mssql_schema
from heeranjid.sql import mssql as base_sql


def install_schema(cur):
    """v0.2.x base + v0.3.1 desc."""
    print("→ Installing v0.2.x base schema...")
    for sql in [
        base_sql.SCHEMA,
        base_sql.SESSION,
        base_sql.GENERATE_HEERID,
        base_sql.GENERATE_RANJID,
        base_sql.CONFIGURE,
        base_sql.SEED,
    ]:
        for batch in sql.split("\nGO\n"):
            batch = batch.strip()
            if batch and batch != "GO":
                cur.execute(batch)

    print("→ Installing v0.3.1 desc surface...")
    for blob in (
        mssql_schema.DESC_FLIP_TSQL,
        mssql_schema.DESC_GENERATORS_TSQL,
        mssql_schema.BULK_BACKFILL_TSQL,
    ):
        for batch in blob.split("GO"):
            if batch.strip():
                cur.execute(batch)

    cur.execute("EXEC heer_set_node_id @node_id = 1")


def seed_rows(cur, table, count):
    print(f"→ Seeding {count} rows into {table}...")
    for _ in range(count):
        cur.execute("EXEC dbo.generate_id @in_node_id = 1")
        raw = int(cur.fetchone()[0])
        cur.execute(f"INSERT INTO {table} (id) VALUES (?)", [raw])


def run_migration(cur, table):
    src_col = "id"
    dst_col = "id_new"

    print(f"→ [1/6] ADD COLUMN {dst_col} bigint NULL")
    cur.execute(f"ALTER TABLE {table} ADD {dst_col} bigint NULL")

    print("→ [2/6] Install autofill trigger")
    trig_sql = mssql_schema.mssql_install_autofill_trigger_for_table(
        table, [(src_col, dst_col)], "heer"
    )
    for batch in trig_sql.split("GO"):
        if batch.strip():
            cur.execute(batch)

    print("→ [3/6] EXEC heeranjid_bulk_backfill")
    cur.execute(
        "EXEC dbo.heeranjid_bulk_backfill "
        "@table_name = ?, @src_col = ?, @dst_col = ?, "
        "@kind = 'heer', @batch_size = ?",
        [table, src_col, dst_col, 1000],
    )

    print("→ [4/6] ALTER COLUMN NOT NULL")
    cur.execute(
        f"ALTER TABLE {table} ALTER COLUMN {dst_col} bigint NOT NULL"
    )

    print("→ [5/6] Cutover: drop old PK, rename, add new PK")
    # Find PK name dynamically
    cur.execute(
        f"SELECT name FROM sys.key_constraints "
        f"WHERE parent_object_id = OBJECT_ID('{table}') AND type = 'PK'"
    )
    (pk_name,) = cur.fetchone()
    cur.execute(f"ALTER TABLE {table} DROP CONSTRAINT {pk_name}")
    cur.execute(f"ALTER TABLE {table} DROP COLUMN {src_col}")
    cur.execute(
        f"EXEC sp_rename '{table}.{dst_col}', '{src_col}', 'COLUMN'"
    )
    cur.execute(f"ALTER TABLE {table} ADD PRIMARY KEY ({src_col})")

    print("→ [6/6] Drop autofill trigger (stale after rename)")
    drop_sql = mssql_schema.mssql_drop_autofill_trigger_for_table(table)
    for batch in drop_sql.split("GO"):
        if batch.strip():
            cur.execute(batch)


def verify(cur, table, row_count):
    print("→ Verifying post-migration state")

    cur.execute(f"SELECT COUNT(*) FROM {table}")
    (c,) = cur.fetchone()
    assert c == row_count, f"row count drifted: expected {row_count}, got {c}"
    print(f"   ✓ row count preserved: {c}")

    cur.execute(f"SELECT TOP 5 id FROM {table} ORDER BY id ASC")
    first5 = [int(r[0]) for r in cur.fetchall()]
    # id is now desc-encoded — ORDER BY id ASC is reverse-chronological.
    # Decoding via HeerIdDesc must give decreasing timestamps.
    timestamps = [HeerIdDesc(v).timestamp_ms for v in first5]
    assert timestamps == sorted(timestamps, reverse=True), (
        f"ORDER BY id ASC should be reverse-chronological; got {timestamps}"
    )
    print(f"   ✓ ORDER BY id ASC is reverse-chronological: first 5 ts = {timestamps}")


def main():
    print(f"Connecting to MSSQL at {MSSQL_URL!r}...")
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()

    cur.execute(
        "IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_example') "
        "CREATE DATABASE heeranjid_example"
    )
    cur.execute("USE heeranjid_example")

    install_schema(cur)

    table = "products_example"
    # Fresh start
    cur.execute(f"IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table}")
    cur.execute(f"CREATE TABLE {table} (id bigint PRIMARY KEY)")

    ROWS = 200
    seed_rows(cur, table, ROWS)

    # Ensure distinct timestamps so the reverse-chronological check is
    # meaningful. generate_id on MSSQL uses wall-clock ms so a tight
    # loop might collide into one ms; sleep briefly between batches.
    time.sleep(0.1)

    print()
    run_migration(cur, table)
    print()

    verify(cur, table, ROWS)

    # Cleanup
    cur.execute(f"DROP TABLE {table}")
    print("\nMigration example completed successfully.")


if __name__ == "__main__":
    main()
