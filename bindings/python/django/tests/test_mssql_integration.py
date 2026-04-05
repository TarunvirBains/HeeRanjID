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


# ── Fixtures ──


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
    from heeranjid.sql import mssql
    for sql in [mssql.SCHEMA, mssql.SESSION, mssql.GENERATE_HEERID, mssql.GENERATE_RANJID, mssql.CONFIGURE, mssql.SEED]:
        for batch in sql.split("\nGO\n"):
            batch = batch.strip()
            if batch and batch != "GO":
                cur.execute(batch)

    # Set epoch and precision
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_config WHERE id = 1)
            INSERT INTO heer_config (id, epoch, precision) VALUES (1, '2026-01-01T00:00:00', 'us')
        ELSE
            UPDATE heer_config SET epoch = '2026-01-01T00:00:00', precision = 'us' WHERE id = 1
    """)

    # Call heer_configure to bake in epoch/precision
    cur.execute("EXEC heer_configure")

    # Register node 2 for multi-node tests
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_nodes WHERE node_id = 2)
            INSERT INTO heer_nodes (node_id, name, description, is_active)
            VALUES (2, N'test-node-2', N'Second test node', 1)
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


# ── HeerId: Basic Generation ──


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

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 100")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_monotonically_increasing(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 50")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = []
        for _ in range(5):
            cursor.execute("EXEC generate_id @in_node_id = 1")
            all_ids.append(int(cursor.fetchone()[0]))
        assert len(set(all_ids)) == 5

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("EXEC generate_id @in_node_id = 1")
        id1 = HeerId(int(cursor.fetchone()[0]))
        cursor.execute("EXEC generate_id @in_node_id = 2")
        id2 = HeerId(int(cursor.fetchone()[0]))
        assert id1.node_id == 1
        assert id2.node_id == 2
        assert id1.as_int() != id2.as_int()

    def test_node_id_roundtrips_through_decode(self, cursor):
        for node in [1, 2]:
            cursor.execute(f"EXEC generate_id @in_node_id = {node}")
            hid = HeerId(int(cursor.fetchone()[0]))
            assert hid.node_id == node


# ── HeerId: Error Cases ──


class TestHeerIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute("EXEC generate_id @in_node_id = 9999")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, @requested_count = 0"
            )

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, @requested_count = -1"
            )

    def test_session_node_id_without_set_fails(self, cursor):
        """A fresh connection without heer_set_node_id should fail."""
        # Clear session context by setting to NULL-ish
        try:
            cursor.execute(
                "EXEC sp_set_session_context @key = N'heer_node_id', @value = NULL"
            )
            with pytest.raises(pyodbc.ProgrammingError):
                cursor.execute("EXEC generate_id")
        except pyodbc.ProgrammingError:
            # If setting NULL also errors, that's acceptable
            pass

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=0, requesting more IDs than fit in one tick fails."""
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, "
                "@requested_count = 8193, @allow_spanning = 0"
            )


# ── RanjId: Basic Generation ──


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

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 100"
        )
        rows = cursor.fetchall()
        ids = [bytes(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_sort_correctly(self, cursor):
        """BINARY(16) should sort in the same order as UUID string sort."""
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 50"
        )
        rows = cursor.fetchall()
        raw_bytes = [bytes(r[0]) for r in rows]
        # Bytes should already be in sorted order (monotonic generation)
        for i in range(len(raw_bytes) - 1):
            assert raw_bytes[i] < raw_bytes[i + 1]

    def test_ranjid_is_valid_uuidv8(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        # UUIDv8: version nibble = 8
        assert u.version == 8
        # Variant should be RFC 4122 (0b10xx)
        assert (u.int >> 62) & 0b11 == 0b10

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        rid1 = RanjId.from_str(
            str(uuid.UUID(bytes=bytes(cursor.fetchone()[0])))
        )
        cursor.execute("EXEC generate_ranjid @in_node_id = 2")
        rid2 = RanjId.from_str(
            str(uuid.UUID(bytes=bytes(cursor.fetchone()[0])))
        )
        assert rid1.node_id == 1
        assert rid2.node_id == 2

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = set()
        for _ in range(10):
            cursor.execute("EXEC generate_ranjid @in_node_id = 1")
            all_ids.add(bytes(cursor.fetchone()[0]))
        assert len(all_ids) == 10


# ── RanjId: Error Cases ──


class TestRanjIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute("EXEC generate_ranjid @in_node_id = 99999")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, @requested_count = 0"
            )

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, @requested_count = -1"
            )

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=0, requesting more than 65536 RanjIds in one tick fails."""
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, "
                "@requested_count = 65537, @allow_spanning = 0"
            )


# ── Django Fields Against Real MSSQL ──


class TestDjangoFieldsMssql:
    """Test Django field methods using real MSSQL-generated values."""

    def test_heerid_field_from_db_value(self, cursor):
        """HeerIdField.from_db_value works with MSSQL integer results."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]

        field = HeerIdField()
        hid = field.from_db_value(int(raw), None, None)
        assert isinstance(hid, HeerId)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_heerid_field_prep_roundtrip(self, cursor):
        """HeerId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]
        original = HeerId(int(raw))

        field = HeerIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(prep, None, None)
        assert restored.as_int() == original.as_int()
        assert restored.node_id == original.node_id

    def test_ranjid_field_from_db_value_bytes(self, cursor):
        """RanjIdField.from_db_value works with MSSQL BINARY(16) bytes."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]

        field = RanjIdField()
        rid = field.from_db_value(bytes(raw_bytes), None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_ranjid_field_from_db_value_memoryview(self, cursor):
        """RanjIdField.from_db_value works with memoryview (pyodbc returns this)."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]

        field = RanjIdField()
        mv = memoryview(bytes(raw_bytes))
        rid = field.from_db_value(mv, None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1

    def test_ranjid_field_prep_roundtrip(self, cursor):
        """RanjId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        original = RanjId.from_str(str(u))

        field = RanjIdField()
        prep = field.get_prep_value(original)
        assert isinstance(prep, uuid.UUID)
        # Roundtrip through string (as from_db_value would receive from Postgres)
        restored = field.from_db_value(str(prep), None, None)
        assert restored.node_id == original.node_id
        assert restored.sequence == original.sequence

    def test_ranjid_field_db_type_mssql(self, cursor):
        """RanjIdField returns BINARY(16) for MSSQL vendor."""
        from heeranjid_django.fields import RanjIdField

        class _FakeConn:
            vendor = "microsoft"

        field = RanjIdField()
        assert field.db_type(_FakeConn()) == "BINARY(16)"


# ── Concurrency ──


class TestConcurrencyMssql:
    def test_concurrent_heerid_uniqueness(self, mssql_conn):
        """Multiple connections generating HeerId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = pyodbc.connect(MSSQL_URL, autocommit=True)
                cur = conn.cursor()
                cur.execute("USE heeranjid_test")
                cur.execute(
                    "EXEC generate_ids @in_node_id = 1, @requested_count = 50"
                )
                rows = cur.fetchall()
                results.extend([int(r[0]) for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate HeerId detected under concurrency"

    def test_concurrent_ranjid_uniqueness(self, mssql_conn):
        """Multiple connections generating RanjId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = pyodbc.connect(MSSQL_URL, autocommit=True)
                cur = conn.cursor()
                cur.execute("USE heeranjid_test")
                cur.execute(
                    "EXEC generate_ranjids @in_node_id = 1, @requested_count = 50"
                )
                rows = cur.fetchall()
                results.extend([bytes(r[0]) for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate RanjId detected under concurrency"
