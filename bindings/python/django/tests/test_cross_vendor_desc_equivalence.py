"""Cross-vendor mask equivalence: MSSQL T-SQL XOR must equal Postgres bytea #.

The Postgres desc encoding uses `bigint #` (64-bit XOR) and `bytea #`
(byte-wise XOR) primitives. The MSSQL v0.3.1 encoding replicates these
with T-SQL `^` on bigint and a byte-loop SUBSTRING+CAST+STUFF
construction on BINARY(16). The outputs must be bit-for-bit identical
so an ID generated on one vendor decodes correctly on the other (and
so a cross-vendor deployment — Django on either — sees consistent
bit patterns).

Requires BOTH DATABASE_URL (Postgres) and MSSQL_URL. If either is
missing, the module is skipped.
"""
import os
import uuid

import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
MSSQL_URL = os.environ.get("MSSQL_URL")

if not DATABASE_URL or not MSSQL_URL:
    pytest.skip(
        "DATABASE_URL and MSSQL_URL both required for cross-vendor "
        "equivalence tests",
        allow_module_level=True,
    )

psycopg2 = pytest.importorskip("psycopg2")
pyodbc = pytest.importorskip("pyodbc")


@pytest.fixture(scope="module")
def pg_conn():
    conn = psycopg2.connect(DATABASE_URL)
    conn.autocommit = True
    yield conn
    conn.close()


@pytest.fixture(scope="module")
def mssql_conn():
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()
    cur.execute(
        "IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_test') "
        "CREATE DATABASE heeranjid_test"
    )
    cur.execute("USE heeranjid_test")
    yield conn
    conn.close()


HEER_TEST_VALUES = [
    0,
    1,
    1_000_000_000,
    (1 << 40) - 1,
    (1 << 40),
    (1 << 62),
    0x7FFFFFFFFFC01FFE,
]


def _to_i64(v):
    """Convert unsigned XOR result back to signed i64."""
    if v >= 2**63:
        return v - 2**64
    return v


class TestHeerIdMaskEquivalence:
    @pytest.mark.parametrize("value", HEER_TEST_VALUES)
    def test_heerid_to_desc_cross_vendor(self, pg_conn, mssql_conn, value):
        # Postgres side
        pg_cur = pg_conn.cursor()
        pg_cur.execute("SELECT heerid_to_desc(%s::bigint)", [value])
        pg_result = pg_cur.fetchone()[0]

        # MSSQL side
        mssql_cur = mssql_conn.cursor()
        mssql_cur.execute(
            "SELECT dbo.heerid_to_desc(CAST(? AS bigint))", [value]
        )
        mssql_result = mssql_cur.fetchone()[0]

        assert pg_result == mssql_result, (
            f"v={value}: PG={pg_result:#x}, MSSQL={mssql_result:#x}"
        )

    @pytest.mark.parametrize("value", HEER_TEST_VALUES)
    def test_heerid_to_asc_is_inverse_on_both_vendors(
        self, pg_conn, mssql_conn, value
    ):
        pg_cur = pg_conn.cursor()
        pg_cur.execute(
            "SELECT heerid_to_asc(heerid_to_desc(%s::bigint))", [value]
        )
        assert pg_cur.fetchone()[0] == _to_i64(value)

        mssql_cur = mssql_conn.cursor()
        mssql_cur.execute(
            "SELECT dbo.heerid_to_asc(dbo.heerid_to_desc(CAST(? AS bigint)))",
            [value],
        )
        assert mssql_cur.fetchone()[0] == _to_i64(value)


RANJ_TEST_VALUES = [
    "00000000-0000-8000-8000-000000000000",
    "11111111-2222-8333-8444-555555555555",
    "ffffffff-ffff-8fff-bfff-ffffffffffff",
    "deadbeef-cafe-8abc-9def-0011223344aa",
]


class TestRanjIdMaskEquivalence:
    @pytest.mark.parametrize("uuid_str", RANJ_TEST_VALUES)
    def test_ranjid_to_desc_cross_vendor(
        self, pg_conn, mssql_conn, uuid_str
    ):
        u = uuid.UUID(uuid_str)

        # Postgres: ranjid_to_desc(uuid) → uuid
        pg_cur = pg_conn.cursor()
        pg_cur.execute("SELECT ranjid_to_desc(%s::uuid)", [str(u)])
        pg_result_uuid = pg_cur.fetchone()[0]
        pg_bytes = (
            pg_result_uuid.bytes
            if isinstance(pg_result_uuid, uuid.UUID)
            else uuid.UUID(str(pg_result_uuid)).bytes
        )

        # MSSQL: dbo.ranjid_to_desc(BINARY(16)) → BINARY(16)
        mssql_cur = mssql_conn.cursor()
        mssql_cur.execute(
            "SELECT dbo.ranjid_to_desc(CAST(? AS BINARY(16)))", [u.bytes]
        )
        mssql_bytes = bytes(mssql_cur.fetchone()[0])

        assert pg_bytes == mssql_bytes, (
            f"u={uuid_str}: PG={pg_bytes.hex()}, MSSQL={mssql_bytes.hex()}"
        )


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
