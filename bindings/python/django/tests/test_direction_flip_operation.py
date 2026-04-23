"""Unit tests for HeeRanjIdDirectionFlip migration operation (no DB)."""

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

from heeranjid_django.operations import HeeRanjIdDirectionFlip


class TestDirectionFlipValidation:
    def test_accepts_heer_to_heer_desc(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        )
        assert op.direction == "heerid_to_heerid_desc"

    def test_accepts_heer_desc_to_heer(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_DESC_TO_HEERID,
        )
        assert op.direction == "heerid_desc_to_heerid"

    def test_accepts_ranj_to_ranj_desc(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_RANJID_TO_RANJID_DESC,
        )
        assert op.direction == "ranjid_to_ranjid_desc"

    def test_accepts_ranj_desc_to_ranj(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_RANJID_DESC_TO_RANJID,
        )
        assert op.direction == "ranjid_desc_to_ranjid"

    def test_rejects_unknown_direction(self):
        with pytest.raises(ValueError, match="Invalid direction"):
            HeeRanjIdDirectionFlip(model="app.Event", direction="not_a_thing")

    def test_rejects_mixed_kind_direction(self):
        """
        heerid_to_ranjid_desc would be a legitimate request but we
        don't support it in this op — that's HeeRanjIdConversion's
        job. Check rejection.
        """
        with pytest.raises(ValueError, match="Invalid direction"):
            HeeRanjIdDirectionFlip(
                model="app.Event", direction="heerid_to_ranjid_desc"
            )


class TestDirectionFlipDeconstruct:
    def test_deconstruct_roundtrip(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        )
        name, args, kwargs = op.deconstruct()
        assert name == "HeeRanjIdDirectionFlip"
        assert args == []
        assert kwargs["model"] == "app.Event"
        assert kwargs["direction"] == "heerid_to_heerid_desc"
        # Reconstruct must not raise.
        HeeRanjIdDirectionFlip(**kwargs)

    def test_deconstruct_omits_default_chunk_size(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        )
        _, _, kwargs = op.deconstruct()
        assert "chunk_size" not in kwargs

    def test_deconstruct_preserves_custom_chunk_size(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
            chunk_size=50000,
        )
        _, _, kwargs = op.deconstruct()
        assert kwargs["chunk_size"] == 50000

    def test_deconstruct_preserves_foreign_keys(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
            foreign_keys=[("child_table", "event_id")],
        )
        _, _, kwargs = op.deconstruct()
        assert kwargs["foreign_keys"] == [("child_table", "event_id")]


class TestDirectionFlipKind:
    def test_heer_directions_return_heer_kind(self):
        for direction in (
            HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
            HeeRanjIdDirectionFlip.DIRECTION_HEERID_DESC_TO_HEERID,
        ):
            op = HeeRanjIdDirectionFlip(model="app.Event", direction=direction)
            assert op._kind() == "heer"

    def test_ranj_directions_return_ranj_kind(self):
        for direction in (
            HeeRanjIdDirectionFlip.DIRECTION_RANJID_TO_RANJID_DESC,
            HeeRanjIdDirectionFlip.DIRECTION_RANJID_DESC_TO_RANJID,
        ):
            op = HeeRanjIdDirectionFlip(model="app.Event", direction=direction)
            assert op._kind() == "ranj"


class TestDirectionFlipStateForwards:
    def test_state_forwards_is_noop(self):
        """
        state_forwards must not mutate project state — schema
        migration is handled by the accompanying AlterField.
        """
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        )
        # No-op call must not raise.
        op.state_forwards("app", None)


class TestDirectionFlipDescribe:
    def test_describe_mentions_direction(self):
        op = HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        )
        assert "heerid_to_heerid_desc" in op.describe()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
