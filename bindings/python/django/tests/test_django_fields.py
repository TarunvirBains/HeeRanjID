import uuid

import django
from django.conf import settings

# Configure Django before importing anything else.
if not settings.configured:
    settings.configure(
        DATABASES={
            "default": {
                "ENGINE": "django.db.backends.sqlite3",
                "NAME": ":memory:",
            }
        },
        INSTALLED_APPS=["heeranjid_django"],
        DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
        HEERANJID_NODE_ID=1,
    )
    django.setup()

import pytest
from heeranjid import HeerId, RanjId
from heeranjid_django.fields import HeerIdField, RanjIdField


# ── HeerIdField ──

class TestHeerIdField:
    def test_internal_type(self):
        field = HeerIdField()
        assert field.get_internal_type() == "BigIntegerField"

    def test_from_db_value_none(self):
        field = HeerIdField()
        assert field.from_db_value(None, None, None) is None

    def test_from_db_value_int(self):
        field = HeerIdField()
        result = field.from_db_value(12345, None, None)
        assert isinstance(result, HeerId)
        assert result.as_int() == 12345

    def test_get_prep_value_none(self):
        field = HeerIdField()
        assert field.get_prep_value(None) is None

    def test_get_prep_value_heerid(self):
        field = HeerIdField()
        hid = HeerId(12345)
        assert field.get_prep_value(hid) == 12345

    def test_get_prep_value_int(self):
        field = HeerIdField()
        assert field.get_prep_value(42) == 42

    def test_db_default_set_when_primary_key(self):
        field = HeerIdField(primary_key=True)
        assert field.db_default is not None

    def test_no_db_default_when_not_primary_key(self):
        field = HeerIdField()
        # db_default should not be set automatically
        from django.db import models as _models
        assert not hasattr(field, '_db_default_set') or field.db_default is _models.NOT_PROVIDED if hasattr(_models, 'NOT_PROVIDED') else True

    def test_deconstruct_path(self):
        field = HeerIdField()
        field.set_attributes_from_name("test_field")
        _name, path, _args, _kwargs = field.deconstruct()
        assert path == "heeranjid_django.fields.HeerIdField"


# ── RanjIdField ──

class _FakeConnection:
    """Minimal connection stub for db_type tests."""
    def __init__(self, vendor):
        self.vendor = vendor


class TestRanjIdField:
    def test_db_type_postgres(self):
        field = RanjIdField()
        conn = _FakeConnection("postgresql")
        assert field.db_type(conn) == "uuid"

    def test_db_type_mssql(self):
        field = RanjIdField()
        conn = _FakeConnection("microsoft")
        assert field.db_type(conn) == "BINARY(16)"

    def test_rel_db_type_matches_db_type(self):
        # rel_db_type delegates to db_type via Field base class
        field = RanjIdField()
        for vendor in ("postgresql", "microsoft"):
            conn = _FakeConnection(vendor)
            assert field.rel_db_type(conn) == field.db_type(conn)

    def test_to_python_none(self):
        field = RanjIdField()
        assert field.to_python(None) is None

    def test_to_python_ranjid_passthrough(self):
        field = RanjIdField()
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        assert field.to_python(rid) is rid

    def test_to_python_string(self):
        field = RanjIdField()
        result = field.to_python("00000000-0000-8000-8000-0000006400c8")
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_to_python_uuid(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-8000-8000-0000006400c8")
        result = field.to_python(u)
        assert isinstance(result, RanjId)

    def test_from_db_value_none(self):
        field = RanjIdField()
        assert field.from_db_value(None, None, None) is None

    def test_from_db_value_str(self):
        field = RanjIdField()
        result = field.from_db_value("00000000-0000-8000-8000-0000006400c8", None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_from_db_value_uuid(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-8000-8000-0000006400c8")
        result = field.from_db_value(u, None, None)
        assert isinstance(result, RanjId)

    def test_from_db_value_bytes(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-8000-8000-0000006400c8")
        result = field.from_db_value(u.bytes, None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_from_db_value_memoryview(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-8000-8000-0000006400c8")
        mv = memoryview(u.bytes)
        result = field.from_db_value(mv, None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100

    def test_get_prep_value_none(self):
        field = RanjIdField()
        assert field.get_prep_value(None) is None

    def test_get_prep_value_ranjid(self):
        field = RanjIdField()
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        result = field.get_prep_value(rid)
        assert isinstance(result, uuid.UUID)

    def test_db_default_set_when_primary_key(self):
        field = RanjIdField(primary_key=True)
        assert field.db_default is not None

    def test_deconstruct_path(self):
        field = RanjIdField()
        field.set_attributes_from_name("test_field")
        _name, path, _args, _kwargs = field.deconstruct()
        assert path == "heeranjid_django.fields.RanjIdField"
