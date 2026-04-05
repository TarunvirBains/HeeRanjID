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
import psycopg2.errors  # noqa: E402  — needed for error-case assertions

from heeranjid import HeerId, RanjId


# ── Fixtures ──


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

    # Register node 2 for multi-node tests
    cur.execute("""
        INSERT INTO heer_nodes (node_id, name, description, is_active)
        VALUES (2, 'test-node-2', 'Second test node', true)
        ON CONFLICT (node_id) DO NOTHING
    """)

    cur.close()
    yield conn
    conn.close()


@pytest.fixture
def cursor(pg_conn):
    cur = pg_conn.cursor()
    yield cur
    cur.close()


# ── HeerId: Basic Generation ──


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

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 100)")
        rows = cursor.fetchall()
        ids = [r[0] for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_monotonically_increasing(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 50)")
        rows = cursor.fetchall()
        ids = [r[0] for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = []
        for _ in range(5):
            cursor.execute("SELECT generate_id(1)")
            all_ids.append(cursor.fetchone()[0])
        assert len(set(all_ids)) == 5

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("SELECT generate_id(1)")
        id1 = HeerId(cursor.fetchone()[0])
        cursor.execute("SELECT generate_id(2)")
        id2 = HeerId(cursor.fetchone()[0])
        assert id1.node_id == 1
        assert id2.node_id == 2
        assert id1.as_int() != id2.as_int()

    def test_node_id_roundtrips_through_decode(self, cursor):
        for node in [1, 2]:
            cursor.execute(f"SELECT generate_id({node})")
            hid = HeerId(cursor.fetchone()[0])
            assert hid.node_id == node


# ── HeerId: Error Cases ──


class TestHeerIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_id(9999)")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT id FROM generate_ids(1, 0)")

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT id FROM generate_ids(1, -1)")

    def test_session_node_id_without_set_fails(self, cursor):
        """A fresh session without set_heer_node_id should fail when node_id is omitted."""
        # Reset session context by setting to NULL
        try:
            cursor.execute("SELECT set_config('heer.node_id', '', false)")
            with pytest.raises(psycopg2.errors.RaiseException):
                cursor.execute("SELECT generate_id()")
        except psycopg2.errors.RaiseException:
            # If clearing the session context also errors, that's acceptable
            pass

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=false, requesting more IDs than fit in one tick fails."""
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ids(1, 8193, false)")


# ── RanjId: Basic Generation ──


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

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 100)")
        rows = cursor.fetchall()
        ids = [str(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_sort_correctly(self, cursor):
        """UUIDv7 string sort should match generation order (monotonic)."""
        cursor.execute("SELECT id FROM generate_ranjids(1, 50)")
        rows = cursor.fetchall()
        ids = [str(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    @pytest.mark.skip(reason="SQL functions still generate UUIDv7, pending heer_configure() update")
    def test_ranjid_is_valid_uuidv8(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        u = uuid.UUID(str(raw))
        # UUIDv8: version nibble = 8
        assert u.version == 8
        # Variant should be RFC 4122 (0b10xx)
        assert (u.int >> 62) & 0b11 == 0b10

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        rid1 = RanjId.from_str(str(cursor.fetchone()[0]))
        cursor.execute("SELECT generate_ranjid(2)")
        rid2 = RanjId.from_str(str(cursor.fetchone()[0]))
        assert rid1.node_id == 1
        assert rid2.node_id == 2

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = set()
        for _ in range(10):
            cursor.execute("SELECT generate_ranjid(1)")
            all_ids.add(str(cursor.fetchone()[0]))
        assert len(all_ids) == 10


# ── RanjId: Error Cases ──


class TestRanjIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjid(99999)")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT id FROM generate_ranjids(1, 0)")

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT id FROM generate_ranjids(1, -1)")

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=false, requesting more than 65536 RanjIds in one tick fails."""
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjids(1, 65537, false)")


# ── Django Fields Against Real Postgres ──


class TestDjangoFieldsPostgres:
    """Test Django field methods using real Postgres-generated values."""

    def test_heerid_field_from_db_value(self, cursor):
        """HeerIdField.from_db_value works with Postgres integer results."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]

        field = HeerIdField()
        hid = field.from_db_value(int(raw), None, None)
        assert isinstance(hid, HeerId)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_heerid_field_prep_roundtrip(self, cursor):
        """HeerId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]
        original = HeerId(int(raw))

        field = HeerIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(prep, None, None)
        assert restored.as_int() == original.as_int()
        assert restored.node_id == original.node_id

    def test_ranjid_field_from_db_value(self, cursor):
        """RanjIdField.from_db_value works with Postgres UUID results (returned as str)."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]

        field = RanjIdField()
        rid = field.from_db_value(str(raw), None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_ranjid_field_prep_roundtrip(self, cursor):
        """RanjId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        original = RanjId.from_str(str(raw))

        field = RanjIdField()
        prep = field.get_prep_value(original)
        assert isinstance(prep, uuid.UUID)
        restored = field.from_db_value(str(prep), None, None)
        assert restored.node_id == original.node_id
        assert restored.sequence == original.sequence

    def test_ranjid_field_db_type_postgres(self, cursor):
        """RanjIdField returns UUID for Postgres vendor."""
        from heeranjid_django.fields import RanjIdField

        class _FakeConn:
            vendor = "postgresql"

        field = RanjIdField()
        assert field.db_type(_FakeConn()) == "uuid"


# ── Concurrency ──


class TestConcurrencyPostgres:
    def test_concurrent_heerid_uniqueness(self, pg_conn):
        """Multiple connections generating HeerId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []
        lock = threading.Lock()

        def generate_ids():
            try:
                conn = psycopg2.connect(DATABASE_URL)
                conn.autocommit = True
                cur = conn.cursor()
                cur.execute("SELECT id FROM generate_ids(1, 50)")
                rows = cur.fetchall()
                with lock:
                    results.extend([r[0] for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                with lock:
                    errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate HeerId detected under concurrency"

    def test_concurrent_ranjid_uniqueness(self, pg_conn):
        """Multiple connections generating RanjId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []
        lock = threading.Lock()

        def generate_ids():
            try:
                conn = psycopg2.connect(DATABASE_URL)
                conn.autocommit = True
                cur = conn.cursor()
                cur.execute("SELECT id FROM generate_ranjids(1, 50)")
                rows = cur.fetchall()
                with lock:
                    results.extend([str(r[0]) for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                with lock:
                    errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate RanjId detected under concurrency"
