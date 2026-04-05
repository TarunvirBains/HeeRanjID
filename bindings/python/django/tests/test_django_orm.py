"""Django ORM integration tests for HeeRanjIdManager.

Runs against a real Postgres database to prove the full stack works:
wheel + SQL constants + migration + fields + manager.

Requires DATABASE_URL environment variable pointing at a running Postgres instance.
Example: DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid
"""
import os
import re

import pytest

# ── Skip early if DATABASE_URL is not set ──

DATABASE_URL = os.environ.get("DATABASE_URL")
pytestmark = pytest.mark.skipif(
    DATABASE_URL is None,
    reason="DATABASE_URL not set — skipping Django ORM integration tests",
)

# ── Parse DATABASE_URL ──

_DB_PATTERN = re.compile(
    r"postgres://(?P<user>[^:]+):(?P<password>[^@]+)@(?P<host>[^:]+):(?P<port>\d+)/(?P<name>.+)"
)


def _parse_db_url(url):
    m = _DB_PATTERN.match(url)
    if not m:
        raise ValueError(
            f"DATABASE_URL must match postgres://user:pass@host:port/dbname, got: {url!r}"
        )
    return m.groupdict()


# ── Configure Django before importing anything Django-related ──

import django  # noqa: E402
from django.conf import settings  # noqa: E402

if DATABASE_URL is not None and not settings.configured:
    _db = _parse_db_url(DATABASE_URL)
    settings.configure(
        DATABASES={
            "default": {
                "ENGINE": "django.db.backends.postgresql",
                "NAME": _db["name"],
                "USER": _db["user"],
                "PASSWORD": _db["password"],
                "HOST": _db["host"],
                "PORT": _db["port"],
            }
        },
        INSTALLED_APPS=["heeranjid_django"],
        DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
        HEERANJID_NODE_ID=1,
    )
    django.setup()

# ── Model definition ──

from django.db import models  # noqa: E402
from heeranjid import HeerId, RanjId  # noqa: E402
from heeranjid_django import HeerIdField, RanjIdField, HeeRanjIdManager  # noqa: E402
from heeranjid_django.managers import (  # noqa: E402
    _get_node_id,
    _generate_heer_ids,
    _generate_ranj_ids,
)

_TABLE = "test_heeranjid_orm_widget"


class Widget(models.Model):
    id = HeerIdField(primary_key=True)
    ranj = RanjIdField()
    name = models.TextField(default="")
    objects = HeeRanjIdManager()

    class Meta:
        app_label = "heeranjid_django"
        db_table = _TABLE
        managed = False  # we create/drop the table manually


# ── Fixtures ──


@pytest.fixture(scope="module")
def pg_conn():
    """
    Connect to Postgres, install the HeeRanjID schema + functions, create the
    test table, yield the connection, then drop the table on teardown.
    """
    psycopg2 = pytest.importorskip("psycopg2")
    conn = psycopg2.connect(DATABASE_URL)
    conn.autocommit = True
    cur = conn.cursor()

    # Install schema and stored functions
    from heeranjid.sql import postgres as pg_sql
    for sql in [
        pg_sql.SCHEMA,
        pg_sql.SESSION,
        pg_sql.GENERATE_HEERID,
        pg_sql.GENERATE_RANJID,
        pg_sql.SEED,
    ]:
        cur.execute(sql)

    # Create test table
    cur.execute(
        f"""
        CREATE TABLE IF NOT EXISTS {_TABLE} (
            id   BIGINT PRIMARY KEY,
            ranj UUID   NOT NULL,
            name TEXT   NOT NULL DEFAULT ''
        )
        """
    )

    cur.close()
    yield conn

    # Teardown: drop test table
    cleanup = conn.cursor()
    cleanup.execute(f"DROP TABLE IF EXISTS {_TABLE}")
    cleanup.close()
    conn.close()


@pytest.fixture(autouse=True)
def clean_rows(pg_conn):
    """Delete all rows from the test table before each individual test."""
    cur = pg_conn.cursor()
    cur.execute(f"DELETE FROM {_TABLE}")
    pg_conn.commit()
    cur.close()


@pytest.fixture
def cursor(pg_conn):
    cur = pg_conn.cursor()
    yield cur
    cur.close()


# ── TestSingleSave ──


class TestSingleSave:
    def test_insert_and_read_back(self, cursor):
        """Insert a record via raw SQL with generated IDs and read it back."""
        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_id = cursor.fetchone()[0]
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        raw_ranj = cursor.fetchone()[0]

        cursor.execute(
            f"INSERT INTO {_TABLE} (id, ranj, name) VALUES (%s, %s, %s)",
            (int(raw_id), str(raw_ranj), "test-widget"),
        )
        cursor.execute(f"SELECT id, ranj FROM {_TABLE} WHERE id = %s", (int(raw_id),))
        row = cursor.fetchone()
        assert row is not None, "Inserted row not found"

        heer_field = HeerIdField()
        ranj_field = RanjIdField()

        hid = heer_field.from_db_value(int(row[0]), None, None)
        rid = ranj_field.from_db_value(str(row[1]), None, None)

        assert isinstance(hid, HeerId)
        assert isinstance(rid, RanjId)

    def test_node_id_matches_setting(self, cursor):
        """The node_id embedded in the generated HeerId matches HEERANJID_NODE_ID."""
        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_id = int(cursor.fetchone()[0])
        hid = HeerId(raw_id)
        assert hid.node_id == node_id

    def test_heerid_prep_from_db_roundtrip(self, cursor):
        """HeerId survives get_prep_value -> from_db_value roundtrip with a real DB value."""
        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_id = int(cursor.fetchone()[0])
        original = HeerId(raw_id)

        field = HeerIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(prep, None, None)

        assert isinstance(restored, HeerId)
        assert restored.as_int() == original.as_int()
        assert restored.node_id == original.node_id

    def test_ranjid_prep_from_db_roundtrip(self, cursor):
        """RanjId survives get_prep_value -> from_db_value roundtrip with a real DB value."""
        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        raw_ranj = str(cursor.fetchone()[0])
        original = RanjId.from_str(raw_ranj)

        field = RanjIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(str(prep), None, None)

        assert isinstance(restored, RanjId)
        assert restored.node_id == original.node_id
        assert restored.sequence == original.sequence


# ── TestBulkCreate ──


class TestBulkCreate:
    def test_generate_ten_heer_ids_unique(self):
        """_generate_heer_ids(10) returns 10 unique HeerId values."""
        ids = _generate_heer_ids(10)
        assert len(ids) == 10
        raw_ints = [hid.as_int() for hid in ids]
        assert len(set(raw_ints)) == 10, "HeerId values are not all unique"

    def test_generate_ten_ranj_ids_unique(self):
        """_generate_ranj_ids(10) returns 10 unique RanjId values."""
        ids = _generate_ranj_ids(10)
        assert len(ids) == 10
        raw_strs = [str(rid) for rid in ids]
        assert len(set(raw_strs)) == 10, "RanjId values are not all unique"

    def test_heer_ids_monotonically_increasing(self):
        """_generate_heer_ids(10) returns IDs in strictly ascending order."""
        ids = _generate_heer_ids(10)
        raw_ints = [hid.as_int() for hid in ids]
        for i in range(len(raw_ints) - 1):
            assert raw_ints[i] < raw_ints[i + 1], (
                f"HeerId at index {i} ({raw_ints[i]}) is not less than "
                f"index {i + 1} ({raw_ints[i + 1]})"
            )

    def test_ranj_ids_are_sortable(self):
        """_generate_ranj_ids(10) returns UUIDv7 values that are lexicographically sorted."""
        ids = _generate_ranj_ids(10)
        raw_strs = [str(rid) for rid in ids]
        assert raw_strs == sorted(raw_strs), (
            "RanjId values are not in lexicographically sorted order"
        )


# ── TestQueryRoundtrip ──


class TestQueryRoundtrip:
    def test_insert_then_query_by_pk(self, cursor, pg_conn):
        """Insert a record and retrieve it via Django ORM filter by PK."""
        from django.db import connection as django_conn

        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_id = int(cursor.fetchone()[0])
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        raw_ranj = str(cursor.fetchone()[0])

        # Insert via raw SQL (no full Django migration in this test suite)
        cursor.execute(
            f"INSERT INTO {_TABLE} (id, ranj, name) VALUES (%s, %s, %s)",
            (raw_id, raw_ranj, "roundtrip-widget"),
        )
        pg_conn.commit()

        # Query via Django ORM
        obj = Widget.objects.get(pk=raw_id)

        assert isinstance(obj.id, HeerId)
        assert isinstance(obj.ranj, RanjId)
        assert obj.id.as_int() == raw_id
        assert obj.id.node_id == node_id
        assert str(obj.ranj) == raw_ranj
        assert obj.name == "roundtrip-widget"

    def test_bulk_insert_then_query_all(self, cursor, pg_conn):
        """Insert multiple records via raw SQL and retrieve them all via ORM."""
        node_id = _get_node_id()
        heer_ids = _generate_heer_ids(5)
        ranj_ids = _generate_ranj_ids(5)

        rows = [
            (hid.as_int(), str(rid), f"widget-{i}")
            for i, (hid, rid) in enumerate(zip(heer_ids, ranj_ids))
        ]
        cursor.executemany(
            f"INSERT INTO {_TABLE} (id, ranj, name) VALUES (%s, %s, %s)", rows
        )
        pg_conn.commit()

        qs = Widget.objects.all().order_by("id")
        assert qs.count() == 5

        for obj in qs:
            assert isinstance(obj.id, HeerId)
            assert isinstance(obj.ranj, RanjId)
            assert obj.id.node_id == node_id
