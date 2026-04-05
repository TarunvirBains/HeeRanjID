# HeeRanjIdManager Design

## Goal

Ensure every Django model with a `HeerIdField` or `RanjIdField` can safely use `bulk_create()`. Provide a manager that generates IDs in batch via SQL, and enforce at class definition time that every model with these fields uses a compliant manager.

## Problem

Django's `bulk_create()` does not call `pre_save()` on fields. The current `HeerIdField` and `RanjIdField` rely on `pre_save()` to generate IDs via SQL. If a user calls `bulk_create()`, the ID fields are inserted as NULL — which silently corrupts data.

Additionally, `pre_save()` hardcodes `node_id = 1` instead of reading from Django settings (which should source it from the environment per the project spec).

## Design

### Components

**`HeeRanjIdPKMixin`** — a manager mixin that adds:
- `heeranjid_bulk_create(objs, **kwargs)` — generates IDs in batch, assigns them to objects, then delegates to Django's `bulk_create()`
- `_heeranjid_enabled = True` — marker attribute used by field enforcement

**`HeeRanjIdManager`** — `HeeRanjIdPKMixin` + `models.Manager`. Drop-in for models that don't need a custom manager.

**Field enforcement** — both `HeerIdField` and `RanjIdField` override `contribute_to_class()`. After the field is attached to the model, it checks that the model's default manager has `_heeranjid_enabled = True`. If not, raises `django.core.exceptions.ImproperlyConfigured` with a message explaining the requirement.

**`HEERANJID_NODE_ID` setting** — both `pre_save()` and `heeranjid_bulk_create()` read the node ID from `django.conf.settings.HEERANJID_NODE_ID`. Users set this in their `settings.py`:

```python
import os
HEERANJID_NODE_ID = int(os.environ["NODE_ID"])
```

If the setting is missing, both `pre_save()` and `heeranjid_bulk_create()` raise `ImproperlyConfigured`.

### `heeranjid_bulk_create` flow

```
1. Inspect model's fields for HeerIdField and RanjIdField instances
2. For each HeerIdField: count objects where the field value is None
3. For each RanjIdField: count objects where the field value is None
4. If HeerId count > 0:
   - Postgres: SELECT id FROM generate_ids(node_id, count)
   - MSSQL: EXEC generate_ids @in_node_id = node_id, @requested_count = count
   - Assign returned IDs to objects in order
5. If RanjId count > 0:
   - Postgres: SELECT id FROM generate_ranjids(node_id, count)
   - MSSQL: EXEC generate_ranjids @in_node_id = node_id, @requested_count = count
   - Assign returned IDs to objects in order
6. Call super().bulk_create(objs, **kwargs)
```

The backend is determined by `connection.vendor` — same pattern as `pre_save()` and the migration.

### `pre_save` fix

Both `HeerIdField.pre_save()` and `RanjIdField.pre_save()` change from:

```python
cursor.execute("SELECT generate_id()")  # hardcoded node_id=1
```

to:

```python
from django.conf import settings
node_id = getattr(settings, 'HEERANJID_NODE_ID', None)
if node_id is None:
    raise ImproperlyConfigured(
        "HEERANJID_NODE_ID must be set in Django settings."
    )
# Postgres:
cursor.execute(f"SELECT generate_id({node_id})")
# MSSQL:
cursor.execute(f"EXEC generate_id @in_node_id = {node_id}")
```

### Enforcement

`contribute_to_class()` on both fields checks the model's default manager:

```python
def contribute_to_class(self, cls, name, **kwargs):
    super().contribute_to_class(cls, name, **kwargs)
    # Defer check until class is fully constructed
    from django.utils.module_loading import lazy_import
    def check_manager(sender, **kwargs):
        manager = cls._default_manager
        if manager is None or not getattr(manager, '_heeranjid_enabled', False):
            raise ImproperlyConfigured(
                f"Model '{cls.__name__}' has a {self.__class__.__name__} but its "
                f"default manager does not support HeeRanjID bulk operations. "
                f"Use HeeRanjIdManager or add HeeRanjIdPKMixin to your custom manager."
            )
    from django.db.models.signals import class_prepared
    class_prepared.connect(check_manager, sender=cls)
```

The check uses `class_prepared` signal to defer until the model class is fully constructed (manager may not be set yet during `contribute_to_class`).

### Public API

```python
from heeranjid_django import (
    HeerIdField,          # 64-bit ID field
    RanjIdField,          # 128-bit UUIDv7 field
    HeeRanjIdManager,     # drop-in manager
    HeeRanjIdPKMixin,     # mixin for custom managers
)
```

### Usage

**Simple case:**
```python
from django.db import models
from heeranjid_django import HeerIdField, HeeRanjIdManager

class MyModel(models.Model):
    id = HeerIdField(primary_key=True)
    name = models.CharField(max_length=100)

    objects = HeeRanjIdManager()
```

**Custom manager:**
```python
from django.db import models
from heeranjid_django import HeerIdField, HeeRanjIdPKMixin

class MyManager(HeeRanjIdPKMixin, models.Manager):
    def active(self):
        return self.filter(is_active=True)

class MyModel(models.Model):
    id = HeerIdField(primary_key=True)
    name = models.CharField(max_length=100)

    objects = MyManager()
```

**Bulk create:**
```python
objs = [MyModel(name=f"item-{i}") for i in range(100)]
MyModel.objects.heeranjid_bulk_create(objs)
# All 100 objects now have unique HeerId primary keys
```

### File layout

All code in `bindings/python/django/src/heeranjid_django/`:

```
heeranjid_django/
  __init__.py         # exports HeerIdField, RanjIdField, HeeRanjIdManager, HeeRanjIdPKMixin
  fields.py           # HeerIdField, RanjIdField (updated: contribute_to_class enforcement, pre_save uses HEERANJID_NODE_ID)
  managers.py         # HeeRanjIdPKMixin, HeeRanjIdManager
  apps.py             # unchanged
  migrations/         # unchanged
```

## Testing

### Unit tests (no database):
- `contribute_to_class` raises `ImproperlyConfigured` when model has no compliant manager
- `contribute_to_class` passes when model uses `HeeRanjIdManager`
- `contribute_to_class` passes when model uses a custom manager with `HeeRanjIdPKMixin`
- `pre_save` raises `ImproperlyConfigured` when `HEERANJID_NODE_ID` is missing
- `pre_save` reads `HEERANJID_NODE_ID` from settings

### Django ORM integration tests (`test_django_orm.py`, dual-backend):

One test file that runs against whichever backend is available (`DATABASE_URL` for Postgres, `MSSQL_URL` for MSSQL). Tests skip when their backend isn't available.

- Apply `0001_install_heeranjid` migration via `schema_editor`
- Create model table with `HeerIdField(primary_key=True)` and `RanjIdField()`
- Single save — `pre_save` generates ID with correct node_id from settings
- Read back — field values are `HeerId` / `RanjId` instances
- `heeranjid_bulk_create` with 10 objects — all get unique IDs
- `heeranjid_bulk_create` — IDs are monotonically increasing (HeerId) / sortable (RanjId)
- Query by primary key roundtrip
- Verify `db_type` matches backend

## What's NOT in scope

- Overriding Django's built-in `bulk_create` (users call `heeranjid_bulk_create` explicitly)
- `bulk_update` support (IDs are immutable after creation)
- Multi-node support within a single Django process (one `HEERANJID_NODE_ID` per process)
- Async manager methods
