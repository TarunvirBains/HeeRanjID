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
from django.db import models
from django.core.exceptions import ImproperlyConfigured


class TestHeeRanjIdPKMixin:
    def test_default_field_is_heerid(self):
        from heeranjid_django import HeeRanjIdPKMixin
        from heeranjid_django.fields import HeerIdField

        class DefaultModel(HeeRanjIdPKMixin, models.Model):
            class Meta:
                app_label = "test_mixin_default"

        pk_field = DefaultModel._meta.get_field("id")
        assert isinstance(pk_field, HeerIdField)
        assert pk_field.primary_key

    def test_ranjid_field_type(self):
        from heeranjid_django import HeeRanjIdPKMixin, HeeRanjIdFieldType
        from heeranjid_django.fields import RanjIdField

        class RanjModel(HeeRanjIdPKMixin, models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID

            class Meta:
                app_label = "test_mixin_ranjid"

        pk_field = RanjModel._meta.get_field("id")
        assert isinstance(pk_field, RanjIdField)
        assert pk_field.primary_key

    def test_has_heeranjid_manager(self):
        from heeranjid_django import HeeRanjIdPKMixin
        from heeranjid_django.managers import HeeRanjIdManager

        class ManagedModel(HeeRanjIdPKMixin, models.Model):
            class Meta:
                app_label = "test_mixin_manager"

        assert isinstance(ManagedModel.objects, HeeRanjIdManager)

    def test_invalid_field_type_raises(self):
        from heeranjid_django import HeeRanjIdPKMixin

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdFieldType"):
            class BadModel(HeeRanjIdPKMixin, models.Model):
                class HeeRanjId:
                    field_type = "invalid"

                class Meta:
                    app_label = "test_mixin_bad_field"

    def test_invalid_prefetch_raises(self):
        from heeranjid_django import HeeRanjIdPKMixin, HeeRanjIdFieldType

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdPrefetch"):
            class BadModel2(HeeRanjIdPKMixin, models.Model):
                class HeeRanjId:
                    field_type = HeeRanjIdFieldType.HEERID
                    prefetch = "invalid"

                class Meta:
                    app_label = "test_mixin_bad_prefetch"


class TestEnumExports:
    def test_field_type_enum_importable(self):
        from heeranjid_django import HeeRanjIdFieldType
        assert HeeRanjIdFieldType.HEERID.value == "heerid"
        assert HeeRanjIdFieldType.RANJID.value == "ranjid"

    def test_prefetch_enum_importable(self):
        from heeranjid_django import HeeRanjIdPrefetch
        assert HeeRanjIdPrefetch.SAVE.value == "save"
        assert HeeRanjIdPrefetch.INIT.value == "init"
        assert HeeRanjIdPrefetch.MANUAL.value is None
