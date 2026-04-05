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
from django.db import models
from django.core.exceptions import ImproperlyConfigured


class TestHeeRanjIdManagerMixin:
    def test_has_heeranjid_enabled_attr(self):
        from heeranjid_django.managers import HeeRanjIdManagerMixin

        class MyManager(HeeRanjIdManagerMixin, models.Manager):
            pass

        mgr = MyManager()
        assert getattr(mgr, "_heeranjid_enabled", False) is True

    def test_has_heeranjid_bulk_create_method(self):
        from heeranjid_django.managers import HeeRanjIdManagerMixin

        class MyManager(HeeRanjIdManagerMixin, models.Manager):
            pass

        mgr = MyManager()
        assert hasattr(mgr, "heeranjid_bulk_create")
        assert callable(mgr.heeranjid_bulk_create)


class TestHeeRanjIdManager:
    def test_has_heeranjid_enabled_attr(self):
        from heeranjid_django.managers import HeeRanjIdManager

        mgr = HeeRanjIdManager()
        assert getattr(mgr, "_heeranjid_enabled", False) is True

    def test_has_heeranjid_bulk_create_method(self):
        from heeranjid_django.managers import HeeRanjIdManager

        mgr = HeeRanjIdManager()
        assert hasattr(mgr, "heeranjid_bulk_create")

    def test_is_django_manager(self):
        from heeranjid_django.managers import HeeRanjIdManager

        mgr = HeeRanjIdManager()
        assert isinstance(mgr, models.Manager)


class TestFieldEnforcement:
    def test_model_with_heeranjid_manager_passes(self):
        from heeranjid_django import HeerIdField, HeeRanjIdManager

        class GoodModel(models.Model):
            id = HeerIdField(primary_key=True)
            objects = HeeRanjIdManager()

            class Meta:
                app_label = "test_enforcement"

    def test_model_with_mixin_manager_passes(self):
        from heeranjid_django import HeerIdField, HeeRanjIdManagerMixin

        class CustomManager(HeeRanjIdManagerMixin, models.Manager):
            pass

        class GoodModel2(models.Model):
            id = HeerIdField(primary_key=True)
            objects = CustomManager()

            class Meta:
                app_label = "test_enforcement2"

    def test_model_without_compliant_manager_raises(self):
        from heeranjid_django import HeerIdField

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdManager"):
            class BadModel(models.Model):
                id = HeerIdField(primary_key=True)

                class Meta:
                    app_label = "test_enforcement3"

    def test_ranjid_field_without_compliant_manager_raises(self):
        from heeranjid_django import RanjIdField

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdManager"):
            class BadModel2(models.Model):
                rid = RanjIdField()

                class Meta:
                    app_label = "test_enforcement4"


class TestNodeIdSetting:
    def test_get_node_id_returns_setting(self):
        from heeranjid_django.managers import _get_node_id

        assert _get_node_id() == 1

    def test_get_node_id_raises_when_missing(self):
        from heeranjid_django.managers import _get_node_id

        original = settings.HEERANJID_NODE_ID
        try:
            del settings.HEERANJID_NODE_ID
            with pytest.raises(ImproperlyConfigured, match="HEERANJID_NODE_ID"):
                _get_node_id()
        finally:
            settings.HEERANJID_NODE_ID = original


class TestExports:
    def test_manager_importable_from_package(self):
        from heeranjid_django import HeeRanjIdManager
        assert HeeRanjIdManager is not None

    def test_mixin_importable_from_package(self):
        from heeranjid_django import HeeRanjIdManagerMixin
        assert HeeRanjIdManagerMixin is not None
