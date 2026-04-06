"""
Form field and ModelForm tests for HeeRanjID Django integration.

These tests exercise Django's form layer — validation, widget rendering, and
cleaned_data types — without requiring a real database connection.
"""
import uuid

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
from django import forms
from django.test import RequestFactory
from heeranjid import RanjId
from heeranjid_django.fields import HeerIdField, RanjIdField, RanjIdFormField


# ── RanjIdFormField unit tests ──────────────────────────────────────────────

class TestRanjIdFormField:
    def test_accepts_hyphenated_uuid_string(self):
        field = RanjIdFormField()
        result = field.to_python("00000000-0000-8000-8000-0000006400c8")
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_accepts_bare_hex_string(self):
        field = RanjIdFormField()
        result = field.to_python("000000000000800080000000006400c8")
        assert isinstance(result, RanjId)

    def test_accepts_uuid_object(self):
        field = RanjIdFormField()
        u = uuid.UUID("00000000-0000-8000-8000-0000006400c8")
        result = field.to_python(u)
        assert isinstance(result, RanjId)

    def test_returns_none_for_empty_string(self):
        field = RanjIdFormField(required=False)
        assert field.to_python("") is None

    def test_returns_none_for_none(self):
        field = RanjIdFormField(required=False)
        assert field.to_python(None) is None

    def test_raises_for_invalid_input(self):
        from django.core.exceptions import ValidationError
        field = RanjIdFormField()
        with pytest.raises(ValidationError):
            field.to_python("not-a-uuid")

    def test_raises_for_truncated_hex(self):
        from django.core.exceptions import ValidationError
        field = RanjIdFormField()
        with pytest.raises(ValidationError):
            field.to_python("deadbeef")

    def test_required_raises_for_empty(self):
        from django.core.exceptions import ValidationError
        field = RanjIdFormField(required=True)
        with pytest.raises(ValidationError):
            field.clean("")

    def test_round_trip_from_ranjid(self):
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        field = RanjIdFormField()
        # Simulate how a RanjId stored on the model is passed to the form
        result = field.to_python(str(rid))
        assert isinstance(result, RanjId)
        assert result == rid


# ── RanjIdField.formfield() integration ─────────────────────────────────────

class TestRanjIdFieldFormfield:
    def test_formfield_returns_ranjid_form_field(self):
        model_field = RanjIdField()
        form_field = model_field.formfield()
        assert isinstance(form_field, RanjIdFormField)

    def test_formfield_widget_is_uuid_compatible(self):
        model_field = RanjIdField()
        form_field = model_field.formfield()
        # RanjIdFormField inherits Django's UUIDField widget (TextInput)
        assert form_field.widget is not None

    def test_formfield_validates_and_returns_ranjid(self):
        model_field = RanjIdField()
        form_field = model_field.formfield()
        result = form_field.clean("00000000-0000-8000-8000-0000006400c8")
        assert isinstance(result, RanjId)


# ── ModelForm integration ────────────────────────────────────────────────────

class _RanjIdForm(forms.Form):
    """Standalone form using RanjIdFormField directly (no model required)."""
    ranj_id = RanjIdFormField()
    label = forms.CharField(max_length=100)


class TestRanjIdModelForm:
    def test_valid_submission_returns_ranjid(self):
        form = _RanjIdForm(data={
            "ranj_id": "00000000-0000-8000-8000-0000006400c8",
            "label": "hello",
        })
        assert form.is_valid(), form.errors
        assert isinstance(form.cleaned_data["ranj_id"], RanjId)

    def test_invalid_uuid_fails_validation(self):
        form = _RanjIdForm(data={
            "ranj_id": "not-a-uuid",
            "label": "hello",
        })
        assert not form.is_valid()
        assert "ranj_id" in form.errors

    def test_missing_required_field_fails(self):
        form = _RanjIdForm(data={"label": "hello"})
        assert not form.is_valid()
        assert "ranj_id" in form.errors

    def test_cleaned_data_node_id_preserved(self):
        form = _RanjIdForm(data={
            "ranj_id": "00000000-0000-8000-8000-0000006400c8",
            "label": "hello",
        })
        assert form.is_valid()
        rid = form.cleaned_data["ranj_id"]
        assert rid.node_id == 100
        assert rid.sequence == 200

    def test_form_post_via_request_factory(self):
        """Simulate a POST request containing a RanjId value."""
        factory = RequestFactory()
        request = factory.post("/fake/", {
            "ranj_id": "00000000-0000-8000-8000-0000006400c8",
            "label": "from request",
        })
        form = _RanjIdForm(data=request.POST)
        assert form.is_valid(), form.errors
        assert isinstance(form.cleaned_data["ranj_id"], RanjId)

    def test_html_rendering_includes_uuid_input(self):
        """The form renders an <input> tag for the RanjId field."""
        form = _RanjIdForm()
        html = form.as_p()
        assert 'name="ranj_id"' in html
        assert "<input" in html

    def test_prepopulated_form_with_ranjid_instance(self):
        """A form pre-populated with a RanjId value renders without error."""
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        form = _RanjIdForm(initial={"ranj_id": str(rid), "label": "pre"})
        html = form.as_p()
        assert "00000000-0000-8000-8000-0000006400c8" in html


# ── HeerIdField formfield sanity check ──────────────────────────────────────

class TestHeerIdFieldFormfield:
    def test_formfield_returns_integer_field(self):
        model_field = HeerIdField()
        form_field = model_field.formfield()
        # HeerIdField subclasses BigIntegerField — formfield is IntegerField
        assert isinstance(form_field, forms.IntegerField)
