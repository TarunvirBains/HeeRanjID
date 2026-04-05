import django
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

import pytest

from heeranjid_django.operations import HeeRanjIdConversion


class TestHeeRanjIdConversionInit:
    """Tests for HeeRanjIdConversion instantiation and basic methods."""

    def test_instantiation_with_required_params(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        assert op.model == "myapp.Customer"
        assert op.direction == "heerid_to_ranjid"
        assert op.foreign_keys == []
        assert op.chunk_size == 10000

    def test_instantiation_with_all_params(self):
        fks = [("myapp_order", "customer_id"), ("myapp_invoice", "customer_id")]
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="ranjid_to_heerid",
            foreign_keys=fks,
            chunk_size=5000,
        )
        assert op.model == "myapp.Customer"
        assert op.direction == "ranjid_to_heerid"
        assert op.foreign_keys == fks
        assert op.chunk_size == 5000

    def test_invalid_direction_raises_value_error(self):
        with pytest.raises(ValueError, match="Invalid direction"):
            HeeRanjIdConversion(
                model="myapp.Customer",
                direction="invalid_direction",
            )

    def test_describe_heerid_to_ranjid(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        assert op.describe() == "Convert myapp.Customer PK: heerid_to_ranjid"

    def test_describe_ranjid_to_heerid(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="ranjid_to_heerid",
        )
        assert op.describe() == "Convert myapp.Customer PK: ranjid_to_heerid"

    def test_get_table_name(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        assert op._get_table_name() == "myapp_customer"

    def test_get_table_name_camel_case(self):
        op = HeeRanjIdConversion(
            model="myapp.MySpecialModel",
            direction="heerid_to_ranjid",
        )
        assert op._get_table_name() == "myapp_myspecialmodel"

    def test_reversible_is_true(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        assert op.reversible is True

    def test_reduces_to_sql_is_true(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        assert op.reduces_to_sql is True

    def test_deconstruct_minimal(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        name, args, kwargs = op.deconstruct()
        assert name == "HeeRanjIdConversion"
        assert args == []
        assert kwargs == {
            "model": "myapp.Customer",
            "direction": "heerid_to_ranjid",
        }

    def test_deconstruct_with_foreign_keys_and_chunk_size(self):
        fks = [("myapp_order", "customer_id")]
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
            foreign_keys=fks,
            chunk_size=5000,
        )
        name, args, kwargs = op.deconstruct()
        assert kwargs["foreign_keys"] == fks
        assert kwargs["chunk_size"] == 5000

    def test_deconstruct_default_chunk_size_omitted(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
            chunk_size=10000,
        )
        _, _, kwargs = op.deconstruct()
        assert "chunk_size" not in kwargs

    def test_state_forwards_is_noop(self):
        op = HeeRanjIdConversion(
            model="myapp.Customer",
            direction="heerid_to_ranjid",
        )
        # Should not raise
        op.state_forwards("myapp", None)

    def test_direction_constants(self):
        assert HeeRanjIdConversion.DIRECTION_HEERID_TO_RANJID == "heerid_to_ranjid"
        assert HeeRanjIdConversion.DIRECTION_RANJID_TO_HEERID == "ranjid_to_heerid"
