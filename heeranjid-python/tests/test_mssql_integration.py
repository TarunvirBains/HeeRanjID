"""Integration tests for HeeRanjID against a real MSSQL database.

Requires MSSQL_URL environment variable.
Start the database: docker compose up mssql -d
"""
import os
import uuid

import pytest

MSSQL_URL = os.environ.get("MSSQL_URL")
if MSSQL_URL is None:
    pytest.fail(
        "MSSQL_URL not set — run 'docker compose up mssql -d' "
        "and set MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};"
        "SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;"
        "TrustServerCertificate=yes'",
        pytrace=False,
    )

pyodbc = pytest.importorskip("pyodbc")

from heeranjid import HeerId, RanjId


@pytest.fixture(scope="module")
def mssql_conn():
    """Connect to MSSQL and install schema."""
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()

    # Create test database if needed
    cur.execute("""
        IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_test')
            CREATE DATABASE heeranjid_test
    """)
    cur.execute("USE heeranjid_test")

    # Install schema and procedures
    from importlib import resources

    sql_dir = resources.files("heeranjid.sql").joinpath("mssql")
    for filename in [
        "schema.sql",
        "session.sql",
        "generate_heerid.sql",
        "generate_ranjid.sql",
        "seed.sql",
    ]:
        sql = sql_dir.joinpath(filename).read_text(encoding="utf-8")
        for batch in sql.split("\nGO\n"):
            batch = batch.strip()
            if batch and batch != "GO":
                cur.execute(batch)

    # Set epoch
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_config WHERE id = 1)
            INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ELSE
            UPDATE heer_config SET epoch = '2024-01-01T00:00:00' WHERE id = 1
    """)

    cur.close()
    yield conn
    conn.close()


@pytest.fixture
def cursor(mssql_conn):
    cur = mssql_conn.cursor()
    cur.execute("USE heeranjid_test")
    yield cur
    cur.close()


class TestHeerIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 10")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(int(r[0])) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_node_id @node_id = 1")
        cursor.execute("EXEC generate_id")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id == 1


class TestRanjIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 10"
        )
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [
            RanjId.from_str(str(uuid.UUID(bytes=bytes(r[0])))) for r in rows
        ]
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_ranj_node_id @node_id = 1")
        cursor.execute("EXEC generate_ranjid")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1
