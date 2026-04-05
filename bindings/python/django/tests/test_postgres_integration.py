"""Integration tests for HeeRanjID against a real Postgres database.

Requires DATABASE_URL environment variable.
Start the database: docker compose up postgres -d
"""
import os
import uuid

import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
if DATABASE_URL is None:
    pytest.fail(
        "DATABASE_URL not set — run 'docker compose up postgres -d' "
        "and set DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid",
        pytrace=False,
    )

psycopg2 = pytest.importorskip("psycopg2")

from heeranjid import HeerId, RanjId


@pytest.fixture(scope="module")
def pg_conn():
    """Connect to Postgres and install schema."""
    conn = psycopg2.connect(DATABASE_URL)
    conn.autocommit = True
    cur = conn.cursor()

    # Install schema and functions
    from heeranjid.sql import postgres
    for sql in [postgres.SCHEMA, postgres.SESSION, postgres.GENERATE_HEERID, postgres.GENERATE_RANJID, postgres.SEED]:
        cur.execute(sql)

    # Set epoch to a known value
    cur.execute("""
        INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ON CONFLICT (id) DO UPDATE SET epoch = EXCLUDED.epoch
    """)

    cur.close()
    yield conn
    conn.close()


@pytest.fixture
def cursor(pg_conn):
    cur = pg_conn.cursor()
    yield cur
    cur.close()


class TestHeerIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_node_id(1)")
        cursor.execute("SELECT generate_id()")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id == 1


class TestRanjIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [RanjId.from_str(str(r[0])) for r in rows]
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_ranj_node_id(1)")
        cursor.execute("SELECT generate_ranjid()")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 1
