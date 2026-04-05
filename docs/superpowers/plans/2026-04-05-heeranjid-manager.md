# HeeRanjIdManager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `HeeRanjIdManager` and `HeeRanjIdManagerMixin` to `heeranjid-django`, enforce compliant managers on models with HeeRanjID fields, fix `pre_save` to read node ID from Django settings, and add Django ORM integration tests.

**Architecture:** New `managers.py` with mixin and manager. Fields gain `contribute_to_class()` enforcement via `class_prepared` signal. Both `pre_save` and `heeranjid_bulk_create` read `HEERANJID_NODE_ID` from `django.conf.settings`. ORM integration tests run against both Postgres and MSSQL backends.

**Tech Stack:** Python 3.10+, Django 4.2+, psycopg2, pyodbc, pytest

---

## File Structure

### Files to create

| File | Purpose |
|------|---------|
| `bindings/python/django/src/heeranjid_django/managers.py` | `HeeRanjIdManagerMixin` and `HeeRanjIdManager` |
| `bindings/python/django/tests/test_managers.py` | Unit tests for manager and enforcement |
| `bindings/python/django/tests/test_django_orm.py` | ORM integration tests (dual-backend) |

### Files to modify

| File | Change |
|------|--------|
| `bindings/python/django/src/heeranjid_django/fields.py` | Add `contribute_to_class` enforcement, fix `pre_save` to use `HEERANJID_NODE_ID` |
| `bindings/python/django/src/heeranjid_django/__init__.py` | Export `HeeRanjIdManager`, `HeeRanjIdManagerMixin` |
| `bindings/python/django/tests/test_django_fields.py` | Update Django config to include `HEERANJID_NODE_ID`, add manager to test models |

---

### Task 1: Create HeeRanjIdManagerMixin and HeeRanjIdManager

**Files:**
- Create: `bindings/python/django/src/heeranjid_django/managers.py`
- Create: `bindings/python/django/tests/test_managers.py`

- [ ] **Step 1: Write the failing tests**

Create `bindings/python/django/tests/test_managers.py`:

```python
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
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py -v
```

Expected: FAIL — `managers` module doesn't exist.

- [ ] **Step 3: Implement managers.py**

Create `bindings/python/django/src/heeranjid_django/managers.py`:

```python
import uuid as uuid_mod

from django.core.exceptions import ImproperlyConfigured
from django.db import connection, models

from heeranjid import HeerId, RanjId


def _get_node_id():
    """Read HEERANJID_NODE_ID from Django settings."""
    from django.conf import settings

    node_id = getattr(settings, "HEERANJID_NODE_ID", None)
    if node_id is None:
        raise ImproperlyConfigured(
            "HEERANJID_NODE_ID must be set in Django settings. "
            "Example: HEERANJID_NODE_ID = int(os.environ['NODE_ID'])"
        )
    return int(node_id)


def _generate_heer_ids(count):
    """Generate a batch of HeerId values via SQL."""
    node_id = _get_node_id()
    cursor = connection.cursor()
    if connection.vendor == "microsoft":
        cursor.execute(
            f"EXEC generate_ids @in_node_id = {node_id}, "
            f"@requested_count = {count}"
        )
    else:
        cursor.execute(f"SELECT id FROM generate_ids({node_id}, {count})")
    rows = cursor.fetchall()
    return [HeerId(int(r[0])) for r in rows]


def _generate_ranj_ids(count):
    """Generate a batch of RanjId values via SQL."""
    node_id = _get_node_id()
    cursor = connection.cursor()
    if connection.vendor == "microsoft":
        cursor.execute(
            f"EXEC generate_ranjids @in_node_id = {node_id}, "
            f"@requested_count = {count}"
        )
        rows = cursor.fetchall()
        return [
            RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(r[0]))))
            for r in rows
        ]
    else:
        cursor.execute(f"SELECT id FROM generate_ranjids({node_id}, {count})")
        rows = cursor.fetchall()
        return [RanjId.from_str(str(r[0])) for r in rows]


class HeeRanjIdManagerMixin:
    """Mixin for Django managers that support HeeRanjID bulk operations."""

    _heeranjid_enabled = True

    def heeranjid_bulk_create(self, objs, **kwargs):
        """Generate HeeRanjID values for objects missing them, then bulk_create."""
        from heeranjid_django.fields import HeerIdField, RanjIdField

        if not objs:
            return self.bulk_create(objs, **kwargs)

        model = self.model

        # Find HeerIdField and RanjIdField instances on the model
        heer_fields = [
            f for f in model._meta.get_fields()
            if isinstance(f, HeerIdField)
        ]
        ranj_fields = [
            f for f in model._meta.get_fields()
            if isinstance(f, RanjIdField)
        ]

        # For each HeerIdField, generate IDs for objects that need them
        for field in heer_fields:
            needs_id = [
                obj for obj in objs
                if getattr(obj, field.attname, None) is None
            ]
            if needs_id:
                ids = _generate_heer_ids(len(needs_id))
                for obj, new_id in zip(needs_id, ids):
                    setattr(obj, field.attname, new_id)

        # For each RanjIdField, generate IDs for objects that need them
        for field in ranj_fields:
            needs_id = [
                obj for obj in objs
                if getattr(obj, field.attname, None) is None
            ]
            if needs_id:
                ids = _generate_ranj_ids(len(needs_id))
                for obj, new_id in zip(needs_id, ids):
                    setattr(obj, field.attname, new_id)

        return self.bulk_create(objs, **kwargs)


class HeeRanjIdManager(HeeRanjIdManagerMixin, models.Manager):
    """Django manager with HeeRanjID bulk create support."""

    pass
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py -v
```

Expected: all 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add bindings/python/django/src/heeranjid_django/managers.py \
        bindings/python/django/tests/test_managers.py
git commit -m "feat(django): add HeeRanjIdManager and HeeRanjIdManagerMixin"
```

---

### Task 2: Add contribute_to_class enforcement to fields

**Files:**
- Modify: `bindings/python/django/src/heeranjid_django/fields.py`
- Modify: `bindings/python/django/tests/test_managers.py`

- [ ] **Step 1: Write the failing tests**

Add these tests to `bindings/python/django/tests/test_managers.py`:

```python
from django.core.exceptions import ImproperlyConfigured


class TestFieldEnforcement:
    def test_model_with_heeranjid_manager_passes(self):
        """Model with HeeRanjIdManager should be accepted."""
        from heeranjid_django import HeerIdField, HeeRanjIdManager

        # This should not raise
        class GoodModel(models.Model):
            id = HeerIdField(primary_key=True)
            objects = HeeRanjIdManager()

            class Meta:
                app_label = "test_enforcement"

    def test_model_with_mixin_manager_passes(self):
        """Model with a custom manager using HeeRanjIdManagerMixin should be accepted."""
        from heeranjid_django import HeerIdField, HeeRanjIdManagerMixin

        class CustomManager(HeeRanjIdManagerMixin, models.Manager):
            pass

        class GoodModel2(models.Model):
            id = HeerIdField(primary_key=True)
            objects = CustomManager()

            class Meta:
                app_label = "test_enforcement2"

    def test_model_without_compliant_manager_raises(self):
        """Model with HeerIdField but no compliant manager should raise."""
        from heeranjid_django import HeerIdField

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdManager"):
            class BadModel(models.Model):
                id = HeerIdField(primary_key=True)

                class Meta:
                    app_label = "test_enforcement3"

    def test_ranjid_field_without_compliant_manager_raises(self):
        """Model with RanjIdField but no compliant manager should raise."""
        from heeranjid_django import RanjIdField

        with pytest.raises(ImproperlyConfigured, match="HeeRanjIdManager"):
            class BadModel2(models.Model):
                rid = RanjIdField()

                class Meta:
                    app_label = "test_enforcement4"
```

- [ ] **Step 2: Run tests to verify enforcement tests fail**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py::TestFieldEnforcement -v
```

Expected: the "passes" tests pass (no enforcement yet), the "raises" tests fail (no exception raised).

- [ ] **Step 3: Add contribute_to_class to both fields**

Modify `bindings/python/django/src/heeranjid_django/fields.py`. Add this method to both `HeerIdField` and `RanjIdField`:

```python
    def contribute_to_class(self, cls, name, **kwargs):
        super().contribute_to_class(cls, name, **kwargs)

        def check_manager(sender, **signal_kwargs):
            manager = cls._default_manager
            if manager is None or not getattr(manager, "_heeranjid_enabled", False):
                raise ImproperlyConfigured(
                    f"Model '{cls.__name__}' has a {self.__class__.__name__} but its "
                    f"default manager does not support HeeRanjID bulk operations. "
                    f"Use HeeRanjIdManager or add HeeRanjIdManagerMixin to your custom manager."
                )

        from django.db.models.signals import class_prepared
        class_prepared.connect(check_manager, sender=cls)
```

Also add the import at the top of `fields.py`:

```python
from django.core.exceptions import ImproperlyConfigured
```

- [ ] **Step 4: Update test_django_fields.py**

The existing field tests define models implicitly (via `HeerIdField()` without a model). But `test_deconstruct_path` calls `field.set_attributes_from_name("test_field")` which doesn't trigger `contribute_to_class`. These tests should still pass as-is since they create standalone field instances, not model classes.

However, the Django settings config block needs `HEERANJID_NODE_ID=1` added for future `pre_save` tests. Update the `settings.configure` call in `test_django_fields.py`:

```python
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
```

- [ ] **Step 5: Run all tests to verify**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py tests/test_django_fields.py -v
```

Expected: all tests pass — enforcement tests raise as expected, existing field tests unaffected.

- [ ] **Step 6: Commit**

```bash
git add bindings/python/django/src/heeranjid_django/fields.py \
        bindings/python/django/tests/test_managers.py \
        bindings/python/django/tests/test_django_fields.py
git commit -m "feat(django): enforce compliant manager on models with HeeRanjID fields"
```

---

### Task 3: Fix pre_save to use HEERANJID_NODE_ID setting

**Files:**
- Modify: `bindings/python/django/src/heeranjid_django/fields.py`
- Modify: `bindings/python/django/tests/test_managers.py`

- [ ] **Step 1: Write the failing tests**

Add these tests to `bindings/python/django/tests/test_managers.py`:

```python
class TestNodeIdSetting:
    def test_get_node_id_returns_setting(self):
        from heeranjid_django.managers import _get_node_id

        # HEERANJID_NODE_ID=1 is set in the test settings.configure above
        assert _get_node_id() == 1

    def test_get_node_id_raises_when_missing(self):
        from heeranjid_django.managers import _get_node_id
        from django.test.utils import override_settings

        with override_settings():
            del settings.HEERANJID_NODE_ID
            with pytest.raises(ImproperlyConfigured, match="HEERANJID_NODE_ID"):
                _get_node_id()
```

- [ ] **Step 2: Run tests to verify they pass**

The `_get_node_id` function was already implemented in Task 1. These tests should pass immediately.

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py::TestNodeIdSetting -v
```

Expected: both tests pass.

- [ ] **Step 3: Update pre_save in HeerIdField**

In `fields.py`, change `HeerIdField.pre_save` from:

```python
    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_id @in_node_id = 1")
        else:
            cursor.execute("SELECT generate_id()")
        row = cursor.fetchone()
        new_id = HeerId(int(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id
```

to:

```python
    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection
        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()
        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute(f"EXEC generate_id @in_node_id = {node_id}")
        else:
            cursor.execute(f"SELECT generate_id({node_id})")
        row = cursor.fetchone()
        new_id = HeerId(int(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id
```

- [ ] **Step 4: Update pre_save in RanjIdField**

Change `RanjIdField.pre_save` from hardcoded `1` to using `_get_node_id()`:

```python
    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection
        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()
        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute(f"EXEC generate_ranjid @in_node_id = {node_id}")
            row = cursor.fetchone()
            raw = row[0]
            new_id = RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(raw))))
        else:
            cursor.execute(f"SELECT generate_ranjid({node_id})")
            row = cursor.fetchone()
            new_id = RanjId.from_str(str(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id
```

- [ ] **Step 5: Run all tests**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/ -v --ignore=tests/test_postgres_integration.py --ignore=tests/test_mssql_integration.py
```

Expected: all unit tests pass.

- [ ] **Step 6: Commit**

```bash
git add bindings/python/django/src/heeranjid_django/fields.py \
        bindings/python/django/tests/test_managers.py
git commit -m "fix(django): read node_id from HEERANJID_NODE_ID setting instead of hardcoding"
```

---

### Task 4: Update __init__.py exports

**Files:**
- Modify: `bindings/python/django/src/heeranjid_django/__init__.py`

- [ ] **Step 1: Update exports**

Change `bindings/python/django/src/heeranjid_django/__init__.py` to:

```python
from heeranjid_django.fields import HeerIdField, RanjIdField
from heeranjid_django.managers import HeeRanjIdManager, HeeRanjIdManagerMixin

default_app_config = "heeranjid_django.apps.HeeranjidConfig"
__all__ = ["HeerIdField", "RanjIdField", "HeeRanjIdManager", "HeeRanjIdManagerMixin"]
```

- [ ] **Step 2: Write import test**

Add to `bindings/python/django/tests/test_managers.py`:

```python
class TestExports:
    def test_manager_importable_from_package(self):
        from heeranjid_django import HeeRanjIdManager
        assert HeeRanjIdManager is not None

    def test_mixin_importable_from_package(self):
        from heeranjid_django import HeeRanjIdManagerMixin
        assert HeeRanjIdManagerMixin is not None
```

- [ ] **Step 3: Run tests**

```bash
cd bindings/python/django && /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_managers.py::TestExports -v
```

Expected: both tests pass.

- [ ] **Step 4: Commit**

```bash
git add bindings/python/django/src/heeranjid_django/__init__.py \
        bindings/python/django/tests/test_managers.py
git commit -m "feat(django): export HeeRanjIdManager and HeeRanjIdManagerMixin"
```

---

### Task 5: Django ORM integration tests

**Files:**
- Create: `bindings/python/django/tests/test_django_orm.py`

- [ ] **Step 1: Write the ORM integration test file**

Create `bindings/python/django/tests/test_django_orm.py`:

```python
"""Django ORM integration tests for HeeRanjID.

Runs against whichever backend is available:
- Set DATABASE_URL for Postgres
- Set MSSQL_URL for MSSQL
- Both can be set to test both backends

Requires: docker compose up postgres -d (and/or mssql)
"""
import os
import uuid

import django
from django.conf import settings

DATABASE_URL = os.environ.get("DATABASE_URL")
MSSQL_URL = os.environ.get("MSSQL_URL")

import pytest

if DATABASE_URL is None and MSSQL_URL is None:
    pytest.fail(
        "Neither DATABASE_URL nor MSSQL_URL is set. "
        "Run 'docker compose up postgres -d' and set DATABASE_URL.",
        pytrace=False,
    )

# Build DATABASES config based on available backends
_databases = {}
if DATABASE_URL:
    # Parse postgres://user:pass@host:port/dbname
    import re

    m = re.match(
        r"postgres://(?P<user>[^:]+):(?P<pass>[^@]+)@(?P<host>[^:]+):(?P<port>\d+)/(?P<db>.+)",
        DATABASE_URL,
    )
    if m:
        _databases["default"] = {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": m.group("db"),
            "USER": m.group("user"),
            "PASSWORD": m.group("pass"),
            "HOST": m.group("host"),
            "PORT": m.group("port"),
        }

if MSSQL_URL:
    _databases["mssql"] = {
        "ENGINE": "mssql",
        "OPTIONS": {"driver": "ODBC Driver 18 for SQL Server"},
    }

if not _databases:
    pytest.fail("Could not configure any database backend.", pytrace=False)

if not settings.configured:
    settings.configure(
        DATABASES=_databases,
        INSTALLED_APPS=["heeranjid_django"],
        DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
        HEERANJID_NODE_ID=1,
    )
    django.setup()

from django.db import connection, models
from heeranjid import HeerId, RanjId
from heeranjid_django import HeerIdField, RanjIdField, HeeRanjIdManager


# ── Test model ──


class TestItem(models.Model):
    id = HeerIdField(primary_key=True)
    rid = RanjIdField()
    name = models.CharField(max_length=100)

    objects = HeeRanjIdManager()

    class Meta:
        app_label = "heeranjid_django"
        db_table = "test_heeranjid_item"


# ── Fixtures ──


@pytest.fixture(scope="module")
def db_setup():
    """Install HeeRanjID schema and create test table."""
    from heeranjid.sql import postgres

    cursor = connection.cursor()

    # Install schema and functions
    for sql in [
        postgres.SCHEMA,
        postgres.SESSION,
        postgres.GENERATE_HEERID,
        postgres.GENERATE_RANJID,
        postgres.SEED,
    ]:
        cursor.execute(sql)

    # Set epoch
    cursor.execute("""
        INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ON CONFLICT (id) DO UPDATE SET epoch = EXCLUDED.epoch
    """)

    # Create test table
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS test_heeranjid_item (
            id BIGINT PRIMARY KEY,
            rid UUID NOT NULL,
            name VARCHAR(100) NOT NULL
        )
    """)

    cursor.close()
    yield

    # Cleanup
    cursor = connection.cursor()
    cursor.execute("DROP TABLE IF EXISTS test_heeranjid_item")
    cursor.close()


@pytest.fixture(autouse=True)
def clean_table(db_setup):
    """Clear the test table before each test."""
    cursor = connection.cursor()
    cursor.execute("DELETE FROM test_heeranjid_item")
    cursor.close()
    yield


# ── Tests ──


class TestSingleSave:
    def test_save_generates_heerid(self, db_setup):
        item = TestItem(name="test-1")
        # Manually simulate pre_save + insert since we bypass Django's ORM save
        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()
        cursor = connection.cursor()
        cursor.execute(f"SELECT generate_id({node_id})")
        hid = HeerId(int(cursor.fetchone()[0]))
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        rid = RanjId.from_str(str(cursor.fetchone()[0]))

        cursor.execute(
            "INSERT INTO test_heeranjid_item (id, rid, name) VALUES (%s, %s, %s)",
            [hid.as_int(), str(rid), "test-1"],
        )

        cursor.execute("SELECT id, rid, name FROM test_heeranjid_item WHERE name = 'test-1'")
        row = cursor.fetchone()
        assert row is not None
        assert HeerId(int(row[0])).node_id == node_id
        cursor.close()

    def test_read_back_returns_correct_types(self, db_setup):
        cursor = connection.cursor()
        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_hid = int(cursor.fetchone()[0])
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        raw_rid = str(cursor.fetchone()[0])

        cursor.execute(
            "INSERT INTO test_heeranjid_item (id, rid, name) VALUES (%s, %s, %s)",
            [raw_hid, raw_rid, "test-read"],
        )

        cursor.execute("SELECT id, rid FROM test_heeranjid_item WHERE name = 'test-read'")
        row = cursor.fetchone()

        field_hid = HeerIdField()
        field_rid = RanjIdField()
        hid = field_hid.from_db_value(row[0], None, None)
        rid = field_rid.from_db_value(str(row[1]), None, None)

        assert isinstance(hid, HeerId)
        assert isinstance(rid, RanjId)
        assert hid.node_id == node_id
        cursor.close()


class TestBulkCreate:
    def test_heeranjid_bulk_create_generates_unique_ids(self, db_setup):
        from heeranjid_django.managers import _generate_heer_ids, _generate_ranj_ids

        heer_ids = _generate_heer_ids(10)
        ranj_ids = _generate_ranj_ids(10)

        assert len(heer_ids) == 10
        assert len(set(h.as_int() for h in heer_ids)) == 10

        assert len(ranj_ids) == 10
        assert len(set(str(r) for r in ranj_ids)) == 10

    def test_heeranjid_bulk_create_monotonic(self, db_setup):
        from heeranjid_django.managers import _generate_heer_ids

        ids = _generate_heer_ids(10)
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_ranjid_bulk_create_sortable(self, db_setup):
        from heeranjid_django.managers import _generate_ranj_ids

        ids = _generate_ranj_ids(10)
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])


class TestQueryRoundtrip:
    def test_pk_roundtrip(self, db_setup):
        cursor = connection.cursor()
        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()
        cursor.execute(f"SELECT generate_id({node_id})")
        raw_hid = int(cursor.fetchone()[0])
        cursor.execute(f"SELECT generate_ranjid({node_id})")
        raw_rid = str(cursor.fetchone()[0])

        cursor.execute(
            "INSERT INTO test_heeranjid_item (id, rid, name) VALUES (%s, %s, %s)",
            [raw_hid, raw_rid, "roundtrip"],
        )

        cursor.execute(
            "SELECT id, rid, name FROM test_heeranjid_item WHERE id = %s",
            [raw_hid],
        )
        row = cursor.fetchone()
        assert row is not None
        assert int(row[0]) == raw_hid
        assert row[2] == "roundtrip"
        cursor.close()
```

- [ ] **Step 2: Run ORM tests against Postgres**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid \
  /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest \
  bindings/python/django/tests/test_django_orm.py -v
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add bindings/python/django/tests/test_django_orm.py
git commit -m "test(django): add ORM integration tests for HeeRanjID manager"
```
