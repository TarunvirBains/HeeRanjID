"""Tests that SQL constants load correctly from heeranjid.sql."""
import pytest


class TestPostgresConstants:
    def test_schema_is_nonempty_string(self):
        from heeranjid.sql.postgres import SCHEMA
        assert isinstance(SCHEMA, str)
        assert len(SCHEMA) > 0
        assert "CREATE TABLE" in SCHEMA

    def test_seed_is_nonempty_string(self):
        from heeranjid.sql.postgres import SEED
        assert isinstance(SEED, str)
        assert len(SEED) > 0

    def test_install_is_nonempty_string(self):
        from heeranjid.sql.postgres import INSTALL
        assert isinstance(INSTALL, str)
        assert len(INSTALL) > 0

    def test_session_is_nonempty_string(self):
        from heeranjid.sql.postgres import SESSION
        assert isinstance(SESSION, str)
        assert len(SESSION) > 0

    def test_generate_heerid_is_nonempty_string(self):
        from heeranjid.sql.postgres import GENERATE_HEERID
        assert isinstance(GENERATE_HEERID, str)
        assert len(GENERATE_HEERID) > 0

    def test_generate_ranjid_is_nonempty_string(self):
        from heeranjid.sql.postgres import GENERATE_RANJID
        assert isinstance(GENERATE_RANJID, str)
        assert len(GENERATE_RANJID) > 0


class TestMssqlConstants:
    def test_schema_is_nonempty_string(self):
        from heeranjid.sql.mssql import SCHEMA
        assert isinstance(SCHEMA, str)
        assert len(SCHEMA) > 0
        assert "CREATE TABLE" in SCHEMA

    def test_seed_is_nonempty_string(self):
        from heeranjid.sql.mssql import SEED
        assert isinstance(SEED, str)
        assert len(SEED) > 0

    def test_install_is_nonempty_string(self):
        from heeranjid.sql.mssql import INSTALL
        assert isinstance(INSTALL, str)
        assert len(INSTALL) > 0

    def test_session_is_nonempty_string(self):
        from heeranjid.sql.mssql import SESSION
        assert isinstance(SESSION, str)
        assert len(SESSION) > 0

    def test_generate_heerid_is_nonempty_string(self):
        from heeranjid.sql.mssql import GENERATE_HEERID
        assert isinstance(GENERATE_HEERID, str)
        assert len(GENERATE_HEERID) > 0

    def test_generate_ranjid_is_nonempty_string(self):
        from heeranjid.sql.mssql import GENERATE_RANJID
        assert isinstance(GENERATE_RANJID, str)
        assert len(GENERATE_RANJID) > 0
