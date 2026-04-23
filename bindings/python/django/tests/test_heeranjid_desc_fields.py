"""Unit tests for HeerIdDescField and RanjIdDescField (no DB required)."""

import uuid

import django
import pytest
from django.conf import settings

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

from heeranjid import HeerIdDesc, RanjIdDesc

from heeranjid_django.fields import (
    HeerIdDescField,
    RanjIdDescField,
    RanjIdDescFormField,
)


class _FakeConnection:
    class _Ops:
        @staticmethod
        def quote_name(value):
            return value

    def __init__(self, vendor, uuid_type="uniqueidentifier"):
        self.vendor = vendor
        self.ops = self._Ops()
        self.data_types = {"UUIDField": uuid_type}


class TestHeerIdDescField:
    def test_internal_type(self):
        field = HeerIdDescField()
        assert field.get_internal_type() == "BigIntegerField"

    def test_from_db_value_none(self):
        field = HeerIdDescField()
        assert field.from_db_value(None, None, None) is None

    def test_from_db_value_int(self):
        field = HeerIdDescField()
        result = field.from_db_value(7, None, None)
        assert isinstance(result, HeerIdDesc)
        assert result.as_int() == 7

    def test_get_prep_value_none(self):
        field = HeerIdDescField()
        assert field.get_prep_value(None) is None

    def test_get_prep_value_heerid_desc(self):
        field = HeerIdDescField()
        hid = HeerIdDesc.from_parts(123_456_789, 1, 0)
        assert field.get_prep_value(hid) == hid.as_int()

    def test_get_prep_value_raw_int(self):
        field = HeerIdDescField()
        assert field.get_prep_value(42) == 42

    def test_deconstruct_roundtrip(self):
        field = HeerIdDescField()
        name, path, args, kwargs = field.deconstruct()
        assert path == "heeranjid_django.fields.HeerIdDescField"
        # Deconstruct → reconstruct must not raise.
        cls = HeerIdDescField
        cls(*args, **kwargs)

    def test_pk_default_uses_heerid_next_desc(self):
        field = HeerIdDescField(primary_key=True)
        # db_default is a Django RawSQL expression wrapping "heerid_next_desc()".
        # Just verify the SQL fragment appears.
        assert "heerid_next_desc" in field.db_default.sql


class TestRanjIdDescField:
    def test_internal_type(self):
        field = RanjIdDescField()
        assert field.get_internal_type() == "UUIDField"

    def test_db_type_microsoft_is_binary_16(self):
        field = RanjIdDescField()
        assert field.db_type(_FakeConnection("microsoft")) == "BINARY(16)"

    def test_db_type_postgres_delegates_to_uuid(self):
        field = RanjIdDescField()
        result = field.db_type(_FakeConnection("postgresql"))
        assert result != "BINARY(16)"

    def test_from_db_value_none(self):
        field = RanjIdDescField()
        assert field.from_db_value(None, None, None) is None

    def test_from_db_value_uuid(self):
        rid = RanjIdDesc.from_parts(999_999, "microseconds", 1, 0)
        u = rid.to_uuid()
        field = RanjIdDescField()
        result = field.from_db_value(u, None, None)
        assert isinstance(result, RanjIdDesc)

    def test_from_db_value_bytes_mssql(self):
        rid = RanjIdDesc.from_parts(999_999, "microseconds", 1, 0)
        raw = rid.to_uuid().bytes  # big-endian 16-byte blob
        field = RanjIdDescField()
        result = field.from_db_value(raw, None, None)
        assert isinstance(result, RanjIdDesc)
        # Round-trip preserves the UUID
        assert result.to_uuid() == rid.to_uuid()

    def test_to_python_passthrough(self):
        rid = RanjIdDesc.from_parts(1000, "microseconds", 1, 0)
        field = RanjIdDescField()
        assert field.to_python(rid) is rid

    def test_to_python_none(self):
        field = RanjIdDescField()
        assert field.to_python(None) is None

    def test_get_prep_value_ranjid_desc(self):
        rid = RanjIdDesc.from_parts(1000, "microseconds", 1, 0)
        field = RanjIdDescField()
        result = field.get_prep_value(rid)
        # Returns uuid.UUID so db drivers can handle it
        assert isinstance(result, uuid.UUID)
        assert result == rid.to_uuid()

    def test_get_prep_value_uuid(self):
        rid = RanjIdDesc.from_parts(2000, "microseconds", 1, 0)
        u = rid.to_uuid()
        field = RanjIdDescField()
        assert field.get_prep_value(u) == u

    def test_deconstruct_roundtrip(self):
        field = RanjIdDescField()
        name, path, args, kwargs = field.deconstruct()
        assert path == "heeranjid_django.fields.RanjIdDescField"
        RanjIdDescField(*args, **kwargs)

    def test_formfield_uses_custom_form_class(self):
        field = RanjIdDescField()
        form_field = field.formfield()
        assert isinstance(form_field, RanjIdDescFormField)

    def test_pk_default_uses_ranjid_next_desc(self):
        field = RanjIdDescField(primary_key=True)
        assert "ranjid_next_desc" in field.db_default.sql


class TestRanjIdDescFormField:
    def test_to_python_valid_uuid_string(self):
        rid = RanjIdDesc.from_parts(1000, "microseconds", 1, 0)
        form_field = RanjIdDescFormField()
        result = form_field.to_python(str(rid.to_uuid()))
        assert isinstance(result, RanjIdDesc)

    def test_to_python_empty_returns_none(self):
        form_field = RanjIdDescFormField()
        assert form_field.to_python("") is None


class TestVendorDispatch:
    """
    pre_save dispatches on connection.vendor to pick the right generator
    procedure. These tests poke at the branch logic without hitting a
    real DB.
    """

    def test_heer_pre_save_microsoft_issues_exec(self):
        # Construct a HeerIdDescField and verify the SQL emitted for
        # microsoft vendor matches the EXEC form.
        # Real pre_save path goes through django.db.connection, which we
        # can't easily mock in-process. Instead verify the field's
        # source references the correct SQL strings.
        import inspect

        src = inspect.getsource(HeerIdDescField.pre_save)
        assert "EXEC heerid_next_desc" in src
        assert "SELECT heerid_next_desc" in src

    def test_ranj_pre_save_microsoft_issues_exec(self):
        import inspect

        src = inspect.getsource(RanjIdDescField.pre_save)
        assert "EXEC ranjid_next_desc" in src
        assert "SELECT ranjid_next_desc" in src


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
