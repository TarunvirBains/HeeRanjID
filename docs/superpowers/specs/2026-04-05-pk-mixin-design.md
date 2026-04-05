# HeeRanjIdPKMixin Design

## Goal

A Django model mixin that provides a HeeRanjID primary key with zero boilerplate. Defaults to HeerId (64-bit). Can be switched to RanjId (128-bit) via a Meta option, which triggers a migration that converts existing IDs and all foreign keys.

## Prerequisites

- `HeeRanjIdManager` and `HeeRanjIdManagerMixin` (implemented)
- `HeerId.batch_to_ranjids` and `RanjId.batch_to_heerids` in Rust core (separate spec: `2026-04-05-id-conversion-design.md`)
- Conversion functions exposed through Python binding

## Usage

**Simple case — 64-bit HeerId (default):**
```python
from django.db import models
from heeranjid_django import HeeRanjIdPKMixin

class Customer(HeeRanjIdPKMixin, models.Model):
    name = models.CharField(max_length=100)
    # Gets: id = HeerIdField(primary_key=True)
    # Gets: objects = HeeRanjIdManager()
```

**128-bit RanjId:**
```python
class Customer(HeeRanjIdPKMixin, models.Model):
    name = models.CharField(max_length=100)

    class HeeRanjId:
        field_type = "ranjid"  # default: "heerid"
```

**Switching from HeerId to RanjId:**
1. Change `field_type = "ranjid"` in the model's `HeeRanjId` inner class
2. Run `manage.py makemigrations` — auto-detects the field change, discovers FKs, generates a `HeeRanjIdConversion` migration operation
3. Review the generated migration, add any FK references the auto-detection missed
4. Run `manage.py migrate` — runs pre-flight overflow check, then converts all IDs

## Components

### `HeeRanjIdPKMixin` (model mixin)

An abstract Django model that:
1. Reads `HeeRanjId.field_type` from the concrete model class (default: `"heerid"`)
2. Sets `id` to either `HeerIdField(primary_key=True)` or `RanjIdField(primary_key=True)`
3. Sets `objects = HeeRanjIdManager()`

```python
class HeeRanjIdPKMixin(models.Model):
    class Meta:
        abstract = True

    class HeeRanjId:
        field_type = "heerid"

    # Field and manager are set dynamically in contribute_to_class
    # based on the concrete model's HeeRanjId.field_type
```

The field is set via `__init_subclass__` or `contribute_to_class` on the mixin, reading the inner class from the concrete model.

### `HeeRanjIdConversion` (migration operation)

A custom `migrations.Operation` subclass that handles the full conversion:

```python
class HeeRanjIdConversion(migrations.Operation):
    def __init__(self, model, direction, foreign_keys):
        """
        model: "app_label.ModelName"
        direction: "heerid_to_ranjid" or "ranjid_to_heerid"
        foreign_keys: [("table_name", "column_name"), ...]
        """
```

**Forward (heerid_to_ranjid):**
1. Pre-flight: no overflow possible (HeerId always fits in RanjId)
2. Add new UUID column to PK table
3. Populate new column using `HeerId.batch_to_ranjids` (fetched from DB, converted in Python via Rust, written back)
4. For each FK: add new UUID column, populate via JOIN on old PK, drop old FK constraint
5. Drop old PK column, rename new column, recreate PK constraint
6. For each FK: drop old column, rename new column, recreate FK constraint

**Reverse (ranjid_to_heerid):**
1. Pre-flight: fetch all RanjIds, run `RanjId.check_heerid_convertibility`. If conflicts, abort with detailed error.
2. Same column-swap pattern as forward, using `RanjId.batch_to_heerids`

### `makemigrations` auto-detection

Django's migration autodetector sees the field type change (BigIntegerField → Field or vice versa). We hook into this via a custom `MigrationAutodetector` or by providing a `deconstruct()` that makes the change detectable.

During `makemigrations`:
1. Detect that a `HeeRanjIdPKMixin` model's field type changed
2. Query Django's `_meta.related_objects` to find all FK references to this model
3. Print the discovered FKs for the user to review
4. Generate a migration file with `HeeRanjIdConversion(model=..., direction=..., foreign_keys=[...])`
5. User reviews and can add missed FKs (e.g., denormalized references in JSON columns)

### Batch processing

For large tables, the conversion processes IDs in chunks to avoid memory issues:

1. `SELECT id FROM table ORDER BY id LIMIT chunk_size OFFSET n`
2. Convert chunk via Rust batch function
3. `UPDATE table SET new_col = %s WHERE id = %s` for each pair
4. Repeat until all rows processed

Default chunk size: 10,000 rows. Configurable via `HeeRanjIdConversion(chunk_size=...)`.

## File Layout

```
heeranjid_django/
  __init__.py         # add HeeRanjIdPKMixin export
  fields.py           # unchanged
  managers.py         # unchanged
  mixins.py           # HeeRanjIdPKMixin
  operations.py       # HeeRanjIdConversion migration operation
  apps.py             # unchanged
  migrations/         # unchanged
```

## Testing

**Unit tests (no database):**
- Model with `HeeRanjIdPKMixin` gets `HeerIdField(primary_key=True)` by default
- Model with `HeeRanjId.field_type = "ranjid"` gets `RanjIdField(primary_key=True)`
- Model with `HeeRanjIdPKMixin` gets `HeeRanjIdManager` as default manager
- Invalid `field_type` raises `ImproperlyConfigured`

**Integration tests (Postgres):**
- Create table with HeerId PK, insert rows
- Run `HeeRanjIdConversion` forward (heerid_to_ranjid)
- Verify all IDs converted correctly, FKs updated
- Run `HeeRanjIdConversion` reverse (ranjid_to_heerid)
- Verify roundtrip preserves data (sequences may differ due to reassignment)

**FK cascade tests:**
- Model A (PK) → Model B (FK to A) → Model C (FK to B)
- Convert A's PK: verify B and C's FK columns also converted
- Query relationships still work after conversion

## What's NOT in scope

- Automatic detection of IDs in JSON or string columns (user must add these manually to the FK list)
- Online/zero-downtime migration (this is an offline schema change)
- Multi-database conversion (one database at a time)
- Conversion of non-PK HeeRanjID fields (only PKs and their FKs)
