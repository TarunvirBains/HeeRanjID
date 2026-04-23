"""Integration tests for v0.3.1 MSSQL descending-sort variants.

Requires MSSQL_URL environment variable; start the container with:
  docker compose up mssql -d

These tests exercise the full MSSQL desc surface: flip functions,
desc generators, bulk backfill, autofill triggers, and the Django
HeeRanjIdDirectionFlip operation against a live MSSQL instance.
"""
import os
import time
import uuid

import pytest

MSSQL_URL = os.environ.get("MSSQL_URL")
if MSSQL_URL is None:
    pytest.skip(
        "MSSQL_URL not set — run 'docker compose up mssql -d' first",
        allow_module_level=True,
    )

pyodbc = pytest.importorskip("pyodbc")

from heeranjid import HeerIdDesc, RanjIdDesc, mssql_schema


@pytest.fixture(scope="module")
def mssql_conn():
    """Connect to MSSQL, install v0.2.x schema + v0.3.1 desc surface."""
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()

    # Create test DB if needed (match test_mssql_integration.py's db).
    cur.execute(
        "IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_test') "
        "CREATE DATABASE heeranjid_test"
    )
    cur.execute("USE heeranjid_test")

    # v0.2.x schema (from bundled heeranjid.sql.mssql).
    from heeranjid.sql import mssql as base_sql

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

    # v0.3.1 desc surface (from the Rust mssql_schema module).
    for blob in [
        mssql_schema.DESC_FLIP_TSQL,
        mssql_schema.DESC_GENERATORS_TSQL,
        mssql_schema.BULK_BACKFILL_TSQL,
    ]:
        for batch in blob.split("GO"):
            if batch.strip():
                cur.execute(batch)

    # Bind a node_id for generator calls.
    cur.execute("EXEC heer_set_node_id @node_id = 1")
    cur.execute("EXEC heer_set_ranj_node_id @node_id = 1")

    yield conn

    # Cleanup left to the module-scope — test DB persists across runs.


# --- Flip functions round-trip -------------------------------------


class TestFlipFunctions:
    def test_heerid_flip_mask_is_expected_value(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("SELECT dbo.heerid_flip_mask()")
        row = cur.fetchone()
        # 0x7FFFFFFFFFC01FFF = 9_223_372_036_850_589_695
        assert row[0] == 0x7FFFFFFFFFC01FFF

    def test_heerid_to_desc_round_trip(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute(
            "SELECT dbo.heerid_to_asc(dbo.heerid_to_desc(CAST(1234567 AS bigint)))"
        )
        assert cur.fetchone()[0] == 1234567

    def test_heerid_to_desc_known_values(self, mssql_conn):
        cur = mssql_conn.cursor()
        # XOR with 0x7FFFFFFFFFC01FFF
        for v in [0, 1, 0x1234567, 0x7FFFFFFFFFC01FFE]:
            cur.execute("SELECT dbo.heerid_to_desc(CAST(? AS bigint))", [v])
            got = cur.fetchone()[0]
            # Unsigned XOR, but SQL returns signed bigint
            expected = v ^ 0x7FFFFFFFFFC01FFF
            # If expected overflows i64, convert
            if expected >= 2**63:
                expected -= 2**64
            assert got == expected, f"v={v}: got={got}, expected={expected}"

    def test_ranjid_to_desc_round_trip(self, mssql_conn):
        cur = mssql_conn.cursor()
        # Seed a known 16-byte value, flip twice, should be identity.
        val = bytes.fromhex("00112233445566778899AABBCCDDEEFF")
        cur.execute(
            "SELECT dbo.ranjid_to_asc(dbo.ranjid_to_desc(CAST(? AS BINARY(16))))",
            [val],
        )
        got = cur.fetchone()[0]
        assert bytes(got) == val


# --- Desc generators -----------------------------------------------


class TestDescGenerators:
    def test_heerid_next_desc_returns_a_row(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("EXEC dbo.heerid_next_desc @in_node_id = 1")
        row = cur.fetchone()
        assert row is not None
        raw = int(row[0])
        # Wrap in HeerIdDesc to verify the bit layout is valid
        hid = HeerIdDesc(raw)
        assert hid.node_id == 1

    def test_heerid_next_desc_has_recent_timestamp(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("EXEC dbo.heerid_next_desc @in_node_id = 1")
        raw = int(cur.fetchone()[0])
        hid = HeerIdDesc(raw)
        # timestamp_ms is logical ms since the configured epoch.
        # Rough sanity: > 0 and < 10 years in ms
        assert 0 < hid.timestamp_ms < 10 * 365 * 24 * 3600 * 1000

    def test_ranjid_next_desc_returns_a_row(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("EXEC dbo.ranjid_next_desc @in_node_id = 1")
        row = cur.fetchone()
        assert row is not None
        raw = bytes(row[0])
        assert len(raw) == 16
        rid = RanjIdDesc.from_str(str(uuid.UUID(bytes=raw)))
        assert rid.node_id == 1


# --- DB sort order -------------------------------------------------


class TestDbSortOrder:
    def test_heerid_desc_sort_matches_reverse_chronological(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("DROP TABLE IF EXISTS desc_sort_test_heer")
        cur.execute(
            "CREATE TABLE desc_sort_test_heer (id_desc bigint PRIMARY KEY)"
        )

        # Generate 5 IDs in sequence. With a time-ordered gen these
        # are chronologically ascending; in desc space they sort
        # physically descending — raw int ordering DESC by time.
        ids = []
        for _ in range(5):
            cur.execute("EXEC dbo.heerid_next_desc @in_node_id = 1")
            raw = int(cur.fetchone()[0])
            ids.append(raw)
            cur.execute(
                "INSERT INTO desc_sort_test_heer (id_desc) VALUES (?)", [raw]
            )
            time.sleep(0.002)  # ensure distinct ms

        cur.execute("SELECT id_desc FROM desc_sort_test_heer ORDER BY id_desc ASC")
        fetched = [int(r[0]) for r in cur.fetchall()]
        # ORDER BY id_desc ASC on desc-encoded values = reverse
        # chronological order = last-inserted first.
        assert fetched == list(reversed(ids))

        cur.execute("DROP TABLE desc_sort_test_heer")


# --- Bulk backfill -------------------------------------------------


class TestBulkBackfill:
    def test_bulk_backfill_populates_desc_column(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("DROP TABLE IF EXISTS backfill_test")
        cur.execute(
            "CREATE TABLE backfill_test ("
            "id bigint PRIMARY KEY, "
            "id_desc bigint NULL)"
        )
        # Seed 50 rows with plain IDs, NULL desc.
        for i in range(50):
            cur.execute("EXEC dbo.generate_id @in_node_id = 1")
            row = cur.fetchone()
            cur.execute(
                "INSERT INTO backfill_test (id, id_desc) VALUES (?, NULL)",
                [int(row[0])],
            )

        # Pre-condition: all id_desc NULL.
        cur.execute("SELECT COUNT(*) FROM backfill_test WHERE id_desc IS NULL")
        assert cur.fetchone()[0] == 50

        # Run backfill.
        cur.execute(
            "EXEC dbo.heeranjid_bulk_backfill "
            "@table_name = N'backfill_test', @src_col = N'id', "
            "@dst_col = N'id_desc', @kind = 'heer', @batch_size = 10"
        )

        # Post-condition: zero NULLs, all desc values match flip(id).
        cur.execute("SELECT COUNT(*) FROM backfill_test WHERE id_desc IS NULL")
        assert cur.fetchone()[0] == 0

        cur.execute(
            "SELECT COUNT(*) FROM backfill_test "
            "WHERE id_desc <> dbo.heerid_to_desc(id)"
        )
        assert cur.fetchone()[0] == 0

        cur.execute("DROP TABLE backfill_test")


# --- Autofill trigger ---------------------------------------------


class TestAutofillTrigger:
    def test_install_autofill_trigger_populates_new_inserts(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("DROP TABLE IF EXISTS trig_test")
        cur.execute(
            "CREATE TABLE trig_test ("
            "id bigint PRIMARY KEY, "
            "id_desc bigint NULL)"
        )

        # Install the autofill trigger via the PyO3-exposed generator.
        trig_sql = mssql_schema.mssql_install_autofill_trigger_for_table(
            "trig_test", [("id", "id_desc")], "heer"
        )
        for batch in trig_sql.split("GO"):
            if batch.strip():
                cur.execute(batch)

        # INSERT with NULL id_desc.
        cur.execute("EXEC dbo.generate_id @in_node_id = 1")
        raw = int(cur.fetchone()[0])
        cur.execute(
            "INSERT INTO trig_test (id, id_desc) VALUES (?, NULL)", [raw]
        )

        # Trigger should have populated id_desc with flip(id).
        cur.execute("SELECT id_desc FROM trig_test WHERE id = ?", [raw])
        id_desc = int(cur.fetchone()[0])
        expected = raw ^ 0x7FFFFFFFFFC01FFF
        if expected >= 2**63:
            expected -= 2**64
        assert id_desc == expected

        # Cleanup
        drop_sql = mssql_schema.mssql_drop_autofill_trigger_for_table("trig_test")
        for batch in drop_sql.split("GO"):
            if batch.strip():
                cur.execute(batch)
        cur.execute("DROP TABLE trig_test")

    def test_install_ranj_autofill_trigger(self, mssql_conn):
        cur = mssql_conn.cursor()
        cur.execute("DROP TABLE IF EXISTS trig_test_ranj")
        cur.execute(
            "CREATE TABLE trig_test_ranj ("
            "id BINARY(16) PRIMARY KEY, "
            "id_desc BINARY(16) NULL)"
        )
        trig_sql = mssql_schema.mssql_install_autofill_trigger_for_table(
            "trig_test_ranj", [("id", "id_desc")], "ranj"
        )
        for batch in trig_sql.split("GO"):
            if batch.strip():
                cur.execute(batch)

        cur.execute("EXEC dbo.generate_ranjid @in_node_id = 1")
        raw = bytes(cur.fetchone()[0])
        cur.execute(
            "INSERT INTO trig_test_ranj (id, id_desc) VALUES (?, NULL)", [raw]
        )

        cur.execute(
            "SELECT id_desc FROM trig_test_ranj WHERE id = ?", [raw]
        )
        id_desc = bytes(cur.fetchone()[0])
        # Expected: byte-wise XOR against 0xFFFFFFFFFFFF0FFF0FFFFFFF8000FFFF
        mask = bytes.fromhex("FFFFFFFFFFFF0FFF0FFFFFFF8000FFFF")
        expected = bytes(a ^ b for a, b in zip(raw, mask))
        assert id_desc == expected

        drop_sql = mssql_schema.mssql_drop_autofill_trigger_for_table(
            "trig_test_ranj"
        )
        for batch in drop_sql.split("GO"):
            if batch.strip():
                cur.execute(batch)
        cur.execute("DROP TABLE trig_test_ranj")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
