# Language/Framework Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the Python package into `heeranjid` (core types + SQL constants) and `heeranjid-django` (Django fields + migrations), and move all binding packages under `bindings/`.

**Architecture:** Move `heeranjid-python/` to `bindings/python/`, extract Django code into `bindings/python/django/` as a separate pure-Python package, add SQL constants module to core, update Cargo workspace paths. Node and .NET move under `bindings/` but their internal structure stays unchanged (framework extraction is future work).

**Tech Stack:** Python 3.10+, maturin (PyO3), hatchling, Django 4.2+, pytest

---

## File Structure

### Files to create

| File | Purpose |
|------|---------|
| `bindings/python/python/heeranjid/sql/postgres/__init__.py` | SQL constants for Postgres (replaces empty file) |
| `bindings/python/python/heeranjid/sql/mssql/__init__.py` | SQL constants for MSSQL (replaces empty file) |
| `bindings/python/tests/test_sql_constants.py` | Tests that SQL constants load correctly |
| `bindings/python/django/pyproject.toml` | heeranjid-django package config |
| `bindings/python/django/src/heeranjid_django/__init__.py` | Exports HeerIdField, RanjIdField |
| `bindings/python/django/src/heeranjid_django/apps.py` | Django AppConfig |
| `bindings/python/django/src/heeranjid_django/fields.py` | HeerIdField, RanjIdField |
| `bindings/python/django/src/heeranjid_django/migrations/__init__.py` | Package marker |
| `bindings/python/django/src/heeranjid_django/migrations/0001_install_heeranjid.py` | Install migration |
| `bindings/python/django/tests/test_django_fields.py` | Django field unit tests |
| `bindings/python/django/tests/test_postgres_integration.py` | Postgres integration tests |
| `bindings/python/django/tests/test_mssql_integration.py` | MSSQL integration tests |

### Files to modify

| File | Change |
|------|--------|
| `Cargo.toml` | Update workspace members paths |
| `bindings/python/Cargo.toml` | Update `heeranjid` dependency path |
| `bindings/python/pyproject.toml` | Remove Django deps and classifiers |
| `bindings/python/Makefile` | Update SQL_SRC path |
| `bindings/node/Cargo.toml` | Update `heeranjid` dependency path |
| `bindings/node/package.json` | Update prepack SQL source path |
| `bindings/node/js/prisma/setup.ts` | Update SQL directory resolution |

### Files to delete (after move)

| File | Reason |
|------|--------|
| `bindings/python/python/heeranjid/django/` | Entire directory — moved to separate package |

---

### Task 1: Move binding packages under `bindings/`

This task uses `git mv` to relocate directories while preserving history. No code changes — just directory moves and path updates.

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Modify: `bindings/python/Cargo.toml` (dependency path)
- Modify: `bindings/python/Makefile` (SQL_SRC path)
- Modify: `bindings/node/Cargo.toml` (dependency path)
- Modify: `bindings/node/package.json` (prepack path)
- Modify: `bindings/node/js/prisma/setup.ts` (SQL directory path)

- [ ] **Step 1: Create bindings directory and move packages**

```bash
mkdir -p bindings
git mv heeranjid-python bindings/python
git mv heeranjid-node bindings/node
git mv heeranjid-dotnet bindings/dotnet
```

- [ ] **Step 2: Update root Cargo.toml workspace members**

Change `Cargo.toml` workspace members from:

```toml
members = ["heeranjid", "heeranjid-sqlx", "heeranjid-python", "heeranjid-node", "heeranjid-ffi"]
```

to:

```toml
members = ["heeranjid", "heeranjid-sqlx", "bindings/python", "bindings/node", "heeranjid-ffi"]
```

- [ ] **Step 3: Update bindings/python/Cargo.toml dependency path**

Change the `heeranjid` dependency path from:

```toml
heeranjid = { path = "../heeranjid", default-features = false }
```

to:

```toml
heeranjid = { path = "../../heeranjid", default-features = false }
```

- [ ] **Step 4: Update bindings/python/Makefile SQL_SRC path**

Change:

```makefile
SQL_SRC := ../sql
```

to:

```makefile
SQL_SRC := ../../sql
```

- [ ] **Step 5: Update bindings/node/Cargo.toml dependency path**

Read `bindings/node/Cargo.toml` and update the `heeranjid` dependency path from `../heeranjid` to `../../heeranjid`.

- [ ] **Step 6: Update bindings/node/package.json prepack path**

Change the prepack script from:

```json
"prepack": "cp -r ../sql ./sql"
```

to:

```json
"prepack": "cp -r ../../sql ./sql"
```

- [ ] **Step 7: Update bindings/node/js/prisma/setup.ts SQL directory resolution**

In `resolveSqlRoot()`, update the submodule fallback path. The bundled path (`join(__dirname, "..", "..", "sql")`) stays the same (it's relative to the npm package). The development fallback changes from `join(__dirname, "..", "..", "..", "sql")` to `join(__dirname, "..", "..", "..", "..", "sql")` (one more `..` because we're now under `bindings/node/`).

- [ ] **Step 8: Verify the workspace compiles**

```bash
cargo check --workspace --exclude heeranjid-python --exclude heeranjid-node
```

Expected: compiles without errors.

- [ ] **Step 9: Verify Rust tests still pass**

```bash
cargo test -p heeranjid --lib
```

Expected: all unit tests pass.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "refactor: move binding packages under bindings/ directory"
```

---

### Task 2: Add SQL constants module to Python core

Replace the empty `__init__.py` files in `heeranjid.sql.postgres` and `heeranjid.sql.mssql` with modules that expose SQL as string constants.

**Files:**
- Modify: `bindings/python/python/heeranjid/sql/postgres/__init__.py`
- Modify: `bindings/python/python/heeranjid/sql/mssql/__init__.py`
- Create: `bindings/python/tests/test_sql_constants.py`

- [ ] **Step 1: Write the failing test**

Create `bindings/python/tests/test_sql_constants.py`:

```python
"""Tests that SQL constants load correctly from heeranjid.sql."""
import pytest


class TestPostgresConstants:
    def test_schema_is_nonempty_string(self):
        from heeranjid.sql.postgres import SCHEMA
        assert isinstance(SCHEMA, str)
        assert len(SCHEMA) > 0
        assert "CREATE TABLE" in SCHEMA

    def test_seed_is_nonempty_string(self):
        from heeranjid.sql.postgres import SEED
        assert isinstance(SEED, str)
        assert len(SEED) > 0

    def test_install_is_nonempty_string(self):
        from heeranjid.sql.postgres import INSTALL
        assert isinstance(INSTALL, str)
        assert len(INSTALL) > 0

    def test_session_is_nonempty_string(self):
        from heeranjid.sql.postgres import SESSION
        assert isinstance(SESSION, str)
        assert len(SESSION) > 0

    def test_generate_heerid_is_nonempty_string(self):
        from heeranjid.sql.postgres import GENERATE_HEERID
        assert isinstance(GENERATE_HEERID, str)
        assert len(GENERATE_HEERID) > 0

    def test_generate_ranjid_is_nonempty_string(self):
        from heeranjid.sql.postgres import GENERATE_RANJID
        assert isinstance(GENERATE_RANJID, str)
        assert len(GENERATE_RANJID) > 0


class TestMssqlConstants:
    def test_schema_is_nonempty_string(self):
        from heeranjid.sql.mssql import SCHEMA
        assert isinstance(SCHEMA, str)
        assert len(SCHEMA) > 0
        assert "CREATE TABLE" in SCHEMA

    def test_seed_is_nonempty_string(self):
        from heeranjid.sql.mssql import SEED
        assert isinstance(SEED, str)
        assert len(SEED) > 0

    def test_install_is_nonempty_string(self):
        from heeranjid.sql.mssql import INSTALL
        assert isinstance(INSTALL, str)
        assert len(INSTALL) > 0

    def test_session_is_nonempty_string(self):
        from heeranjid.sql.mssql import SESSION
        assert isinstance(SESSION, str)
        assert len(SESSION) > 0

    def test_generate_heerid_is_nonempty_string(self):
        from heeranjid.sql.mssql import GENERATE_HEERID
        assert isinstance(GENERATE_HEERID, str)
        assert len(GENERATE_HEERID) > 0

    def test_generate_ranjid_is_nonempty_string(self):
        from heeranjid.sql.mssql import GENERATE_RANJID
        assert isinstance(GENERATE_RANJID, str)
        assert len(GENERATE_RANJID) > 0
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd bindings/python && make dev && pytest tests/test_sql_constants.py -v
```

Expected: FAIL — the `__init__.py` files are empty, so constants don't exist.

- [ ] **Step 3: Implement postgres SQL constants**

Write `bindings/python/python/heeranjid/sql/postgres/__init__.py`:

```python
"""Postgres SQL constants — loaded from bundled .sql files at import time."""
from importlib import resources

_pkg = resources.files(__package__)

try:
    SCHEMA = _pkg.joinpath("schema.sql").read_text(encoding="utf-8")
    SEED = _pkg.joinpath("seed.sql").read_text(encoding="utf-8")
    INSTALL = _pkg.joinpath("install.sql").read_text(encoding="utf-8")
    SESSION = _pkg.joinpath("session.sql").read_text(encoding="utf-8")
    GENERATE_HEERID = _pkg.joinpath("generate_heerid.sql").read_text(encoding="utf-8")
    GENERATE_RANJID = _pkg.joinpath("generate_ranjid.sql").read_text(encoding="utf-8")
except FileNotFoundError:
    raise FileNotFoundError(
        "SQL files not found in heeranjid.sql.postgres. "
        "Build with 'make dev' or 'make build' to copy SQL files from the sql/ submodule."
    )
```

- [ ] **Step 4: Implement mssql SQL constants**

Write `bindings/python/python/heeranjid/sql/mssql/__init__.py`:

```python
"""MSSQL SQL constants — loaded from bundled .sql files at import time."""
from importlib import resources

_pkg = resources.files(__package__)

try:
    SCHEMA = _pkg.joinpath("schema.sql").read_text(encoding="utf-8")
    SEED = _pkg.joinpath("seed.sql").read_text(encoding="utf-8")
    INSTALL = _pkg.joinpath("install.sql").read_text(encoding="utf-8")
    SESSION = _pkg.joinpath("session.sql").read_text(encoding="utf-8")
    GENERATE_HEERID = _pkg.joinpath("generate_heerid.sql").read_text(encoding="utf-8")
    GENERATE_RANJID = _pkg.joinpath("generate_ranjid.sql").read_text(encoding="utf-8")
except FileNotFoundError:
    raise FileNotFoundError(
        "SQL files not found in heeranjid.sql.mssql. "
        "Build with 'make dev' or 'make build' to copy SQL files from the sql/ submodule."
    )
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd bindings/python && pytest tests/test_sql_constants.py -v
```

Expected: all 12 tests pass.

- [ ] **Step 6: Run existing core tests to verify no regressions**

```bash
cd bindings/python && pytest tests/test_heerid.py tests/test_ranjid.py -v
```

Expected: all existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add bindings/python/python/heeranjid/sql/postgres/__init__.py \
        bindings/python/python/heeranjid/sql/mssql/__init__.py \
        bindings/python/tests/test_sql_constants.py
git commit -m "feat(python): add SQL constants module for postgres and mssql"
```

---

### Task 3: Create heeranjid-django package

Create the new `heeranjid-django` pure-Python package under `bindings/python/django/`.

**Files:**
- Create: `bindings/python/django/pyproject.toml`
- Create: `bindings/python/django/src/heeranjid_django/__init__.py`
- Create: `bindings/python/django/src/heeranjid_django/apps.py`
- Create: `bindings/python/django/src/heeranjid_django/fields.py`
- Create: `bindings/python/django/src/heeranjid_django/migrations/__init__.py`
- Create: `bindings/python/django/src/heeranjid_django/migrations/0001_install_heeranjid.py`

- [ ] **Step 1: Create pyproject.toml**

Create `bindings/python/django/pyproject.toml`:

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "heeranjid-django"
version = "0.1.0"
description = "Django integration for HeeRanjID distributed ID generation"
requires-python = ">=3.10"
license = "MIT"
dependencies = [
    "heeranjid",
    "django>=4.2",
]
classifiers = [
    "Framework :: Django",
    "Framework :: Django :: 4.2",
    "Framework :: Django :: 5.2",
]

[project.optional-dependencies]
dev = ["pytest>=8.0"]

[tool.hatch.build.targets.wheel]
packages = ["src/heeranjid_django"]
```

- [ ] **Step 2: Create __init__.py**

Create `bindings/python/django/src/heeranjid_django/__init__.py`:

```python
from heeranjid_django.fields import HeerIdField, RanjIdField

default_app_config = "heeranjid_django.apps.HeeranjidConfig"
__all__ = ["HeerIdField", "RanjIdField"]
```

- [ ] **Step 3: Create apps.py**

Create `bindings/python/django/src/heeranjid_django/apps.py`:

```python
from django.apps import AppConfig


class HeeranjidConfig(AppConfig):
    name = "heeranjid_django"
    verbose_name = "HeeRanjID"
    default_auto_field = "django.db.models.BigAutoField"
```

- [ ] **Step 4: Create fields.py**

Create `bindings/python/django/src/heeranjid_django/fields.py`:

```python
import uuid as uuid_mod

from django.db import models
from django.db.models.expressions import RawSQL
from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_id()", [])
        super().__init__(*args, **kwargs)

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

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        return HeerId(int(value))

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, HeerId):
            return value.as_int()
        return int(value)

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid_django.fields.HeerIdField", args, kwargs


class RanjIdField(models.Field):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_ranjid()", [])
        super().__init__(*args, **kwargs)

    def db_type(self, connection):
        if connection.vendor == "microsoft":
            return "BINARY(16)"
        return "uuid"

    def rel_db_type(self, connection):
        return self.db_type(connection)

    def get_internal_type(self):
        return "RanjIdField"

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_ranjid @in_node_id = 1")
            row = cursor.fetchone()
            raw = row[0]
            new_id = RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(raw))))
        else:
            cursor.execute("SELECT generate_ranjid()")
            row = cursor.fetchone()
            new_id = RanjId.from_str(str(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        if isinstance(value, (bytes, memoryview)):
            value = uuid_mod.UUID(bytes=bytes(value))
        if not isinstance(value, str):
            value = str(value)
        return RanjId.from_str(value)

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, RanjId):
            return value.to_uuid()
        return value

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid_django.fields.RanjIdField", args, kwargs
```

- [ ] **Step 5: Create migrations package**

Create `bindings/python/django/src/heeranjid_django/migrations/__init__.py`:

```python
```

(Empty file — package marker.)

- [ ] **Step 6: Create install migration**

Create `bindings/python/django/src/heeranjid_django/migrations/0001_install_heeranjid.py`:

```python
"""Install HeeRanjID schema and functions/procedures."""
from django.db import migrations


def _get_sql_module(schema_editor):
    """Return the SQL constants module for the current database backend."""
    vendor = schema_editor.connection.vendor
    if vendor == "microsoft":
        from heeranjid.sql import mssql
        return mssql
    from heeranjid.sql import postgres
    return postgres


def forwards(apps, schema_editor):
    sql = _get_sql_module(schema_editor)
    sql_parts = [
        sql.SCHEMA,
        sql.SESSION,
        sql.GENERATE_HEERID,
        sql.GENERATE_RANJID,
        sql.SEED,
    ]

    backend = "mssql" if schema_editor.connection.vendor == "microsoft" else "postgres"
    for part in sql_parts:
        if backend == "mssql":
            # MSSQL requires splitting on GO batch separators
            batches = part.split("\nGO\n")
            for batch in batches:
                batch = batch.strip()
                if batch and batch != "GO":
                    schema_editor.execute(batch)
        else:
            schema_editor.execute(part)


def backwards(apps, schema_editor):
    vendor = schema_editor.connection.vendor

    if vendor == "microsoft":
        drops = [
            "DROP PROCEDURE IF EXISTS generate_id;",
            "DROP PROCEDURE IF EXISTS generate_ids;",
            "DROP PROCEDURE IF EXISTS generate_ranjid;",
            "DROP PROCEDURE IF EXISTS generate_ranjids;",
            "DROP PROCEDURE IF EXISTS heer_set_node_id;",
            "DROP PROCEDURE IF EXISTS heer_set_ranj_node_id;",
            "DROP FUNCTION IF EXISTS dbo.heer_current_node_id;",
            "DROP FUNCTION IF EXISTS dbo.heer_current_ranj_node_id;",
            "DROP TABLE IF EXISTS heer_ranj_node_state;",
            "DROP TABLE IF EXISTS heer_node_state;",
            "DROP TABLE IF EXISTS heer_config;",
            "DROP TABLE IF EXISTS heer_nodes;",
        ]
    else:
        drops = [
            "DROP FUNCTION IF EXISTS generate_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjid() CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjid(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS set_heer_node_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS current_heer_node_id() CASCADE;",
            "DROP FUNCTION IF EXISTS set_heer_ranj_node_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS current_heer_ranj_node_id() CASCADE;",
            "DROP TABLE IF EXISTS heer_ranj_node_state CASCADE;",
            "DROP TABLE IF EXISTS heer_node_state CASCADE;",
            "DROP TABLE IF EXISTS heer_config CASCADE;",
            "DROP TABLE IF EXISTS heer_nodes CASCADE;",
        ]

    for stmt in drops:
        schema_editor.execute(stmt)


class Migration(migrations.Migration):
    initial = True
    dependencies = []
    operations = [
        migrations.RunPython(forwards, backwards),
    ]
```

- [ ] **Step 7: Commit**

```bash
git add bindings/python/django/
git commit -m "feat: create heeranjid-django package"
```

---

### Task 4: Move Django tests to heeranjid-django and update imports

Move the Django-specific tests from the core package to the new Django package and update all imports.

**Files:**
- Create: `bindings/python/django/tests/test_django_fields.py`
- Create: `bindings/python/django/tests/test_postgres_integration.py`
- Create: `bindings/python/django/tests/test_mssql_integration.py`
- Delete: `bindings/python/tests/test_django_fields.py`
- Delete: `bindings/python/tests/test_postgres_integration.py`
- Delete: `bindings/python/tests/test_mssql_integration.py`

- [ ] **Step 1: Move test_django_fields.py with updated imports**

Create `bindings/python/django/tests/test_django_fields.py` with the content from `bindings/python/tests/test_django_fields.py`, changing:

- `from heeranjid.django.fields import HeerIdField, RanjIdField` → `from heeranjid_django.fields import HeerIdField, RanjIdField`
- `INSTALLED_APPS=[]` → `INSTALLED_APPS=["heeranjid_django"]`
- The `test_deconstruct_path` assertions change from `"heeranjid.django.fields.HeerIdField"` to `"heeranjid_django.fields.HeerIdField"` and `"heeranjid.django.fields.RanjIdField"` to `"heeranjid_django.fields.RanjIdField"`

```python
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
        field = RanjIdField()
        for vendor in ("postgresql", "microsoft"):
            conn = _FakeConnection(vendor)
            assert field.rel_db_type(conn) == field.db_type(conn)

    def test_from_db_value_none(self):
        field = RanjIdField()
        assert field.from_db_value(None, None, None) is None

    def test_from_db_value_str(self):
        field = RanjIdField()
        result = field.from_db_value("00000000-0000-7000-800f-4240006400c8", None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_from_db_value_uuid(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-7000-800f-4240006400c8")
        result = field.from_db_value(u, None, None)
        assert isinstance(result, RanjId)

    def test_from_db_value_bytes(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-7000-800f-4240006400c8")
        result = field.from_db_value(u.bytes, None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100
        assert result.sequence == 200

    def test_from_db_value_memoryview(self):
        field = RanjIdField()
        u = uuid.UUID("00000000-0000-7000-800f-4240006400c8")
        mv = memoryview(u.bytes)
        result = field.from_db_value(mv, None, None)
        assert isinstance(result, RanjId)
        assert result.node_id == 100

    def test_get_prep_value_none(self):
        field = RanjIdField()
        assert field.get_prep_value(None) is None

    def test_get_prep_value_ranjid(self):
        field = RanjIdField()
        rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
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
```

- [ ] **Step 2: Move test_postgres_integration.py with updated imports**

Create `bindings/python/django/tests/test_postgres_integration.py` with the content from `bindings/python/tests/test_postgres_integration.py`, changing the SQL loading to use the new constants:

Change the fixture's SQL loading from:

```python
from importlib import resources
sql_dir = resources.files("heeranjid.sql").joinpath("postgres")
for filename in ["schema.sql", "session.sql", "generate_heerid.sql", "generate_ranjid.sql", "seed.sql"]:
    cur.execute(sql_dir.joinpath(filename).read_text(encoding="utf-8"))
```

to:

```python
from heeranjid.sql import postgres
for sql in [postgres.SCHEMA, postgres.SESSION, postgres.GENERATE_HEERID, postgres.GENERATE_RANJID, postgres.SEED]:
    cur.execute(sql)
```

The rest of the file stays the same (it uses `heeranjid.HeerId` and `heeranjid.RanjId`, which don't change).

Full file:

```python
"""Integration tests for HeeRanjID against a real Postgres database.

Requires DATABASE_URL environment variable.
Start the database: docker compose up postgres -d
"""
import os
import uuid

import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
if DATABASE_URL is None:
    pytest.fail(
        "DATABASE_URL not set — run 'docker compose up postgres -d' "
        "and set DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid",
        pytrace=False,
    )

psycopg2 = pytest.importorskip("psycopg2")

from heeranjid import HeerId, RanjId


@pytest.fixture(scope="module")
def pg_conn():
    """Connect to Postgres and install schema."""
    conn = psycopg2.connect(DATABASE_URL)
    conn.autocommit = True
    cur = conn.cursor()

    # Install schema and functions
    from heeranjid.sql import postgres

    for sql in [postgres.SCHEMA, postgres.SESSION, postgres.GENERATE_HEERID, postgres.GENERATE_RANJID, postgres.SEED]:
        cur.execute(sql)

    # Set epoch to a known value
    cur.execute("""
        INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ON CONFLICT (id) DO UPDATE SET epoch = EXCLUDED.epoch
    """)

    cur.close()
    yield conn
    conn.close()


@pytest.fixture
def cursor(pg_conn):
    cur = pg_conn.cursor()
    yield cur
    cur.close()


class TestHeerIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_node_id(1)")
        cursor.execute("SELECT generate_id()")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id == 1


class TestRanjIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [RanjId.from_str(str(r[0])) for r in rows]
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_ranj_node_id(1)")
        cursor.execute("SELECT generate_ranjid()")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 1
```

- [ ] **Step 3: Move test_mssql_integration.py with updated imports**

Create `bindings/python/django/tests/test_mssql_integration.py` with the content from `bindings/python/tests/test_mssql_integration.py`, making these changes:

1. SQL loading in the fixture changes from `importlib.resources` to constants:

```python
from heeranjid.sql import mssql
for sql in [mssql.SCHEMA, mssql.SESSION, mssql.GENERATE_HEERID, mssql.GENERATE_RANJID, mssql.SEED]:
    for batch in sql.split("\nGO\n"):
        batch = batch.strip()
        if batch and batch != "GO":
            cur.execute(batch)
```

2. Django field imports in `TestDjangoFieldsMssql` change from `from heeranjid.django.fields import HeerIdField` to `from heeranjid_django.fields import HeerIdField` (and same for `RanjIdField`).

Full file (fixture and class `TestDjangoFieldsMssql` shown — all other classes are identical to the original):

```python
"""Integration tests for HeeRanjID against a real MSSQL database.

Requires MSSQL_URL environment variable.
Start the database: docker compose up mssql -d
"""
import os
import uuid

import pytest

MSSQL_URL = os.environ.get("MSSQL_URL")
if MSSQL_URL is None:
    pytest.fail(
        "MSSQL_URL not set — run 'docker compose up mssql -d' "
        "and set MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};"
        "SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;"
        "TrustServerCertificate=yes'",
        pytrace=False,
    )

pyodbc = pytest.importorskip("pyodbc")

from heeranjid import HeerId, RanjId


# ── Fixtures ──


@pytest.fixture(scope="module")
def mssql_conn():
    """Connect to MSSQL and install schema."""
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()

    # Create test database if needed
    cur.execute("""
        IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_test')
            CREATE DATABASE heeranjid_test
    """)
    cur.execute("USE heeranjid_test")

    # Install schema and procedures
    from heeranjid.sql import mssql

    for sql in [mssql.SCHEMA, mssql.SESSION, mssql.GENERATE_HEERID, mssql.GENERATE_RANJID, mssql.SEED]:
        for batch in sql.split("\nGO\n"):
            batch = batch.strip()
            if batch and batch != "GO":
                cur.execute(batch)

    # Set epoch
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_config WHERE id = 1)
            INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ELSE
            UPDATE heer_config SET epoch = '2024-01-01T00:00:00' WHERE id = 1
    """)

    # Register node 2 for multi-node tests
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_nodes WHERE node_id = 2)
            INSERT INTO heer_nodes (node_id, name, description, is_active)
            VALUES (2, N'test-node-2', N'Second test node', 1)
    """)

    cur.close()
    yield conn
    conn.close()


@pytest.fixture
def cursor(mssql_conn):
    cur = mssql_conn.cursor()
    cur.execute("USE heeranjid_test")
    yield cur
    cur.close()


# ── HeerId: Basic Generation ──


class TestHeerIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 10")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(int(r[0])) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_node_id @node_id = 1")
        cursor.execute("EXEC generate_id")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id == 1

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 100")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_monotonically_increasing(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 50")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = []
        for _ in range(5):
            cursor.execute("EXEC generate_id @in_node_id = 1")
            all_ids.append(int(cursor.fetchone()[0]))
        assert len(set(all_ids)) == 5

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("EXEC generate_id @in_node_id = 1")
        id1 = HeerId(int(cursor.fetchone()[0]))
        cursor.execute("EXEC generate_id @in_node_id = 2")
        id2 = HeerId(int(cursor.fetchone()[0]))
        assert id1.node_id == 1
        assert id2.node_id == 2
        assert id1.as_int() != id2.as_int()

    def test_node_id_roundtrips_through_decode(self, cursor):
        for node in [1, 2]:
            cursor.execute(f"EXEC generate_id @in_node_id = {node}")
            hid = HeerId(int(cursor.fetchone()[0]))
            assert hid.node_id == node


# ── HeerId: Error Cases ──


class TestHeerIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute("EXEC generate_id @in_node_id = 9999")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, @requested_count = 0"
            )

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, @requested_count = -1"
            )

    def test_session_node_id_without_set_fails(self, cursor):
        """A fresh connection without heer_set_node_id should fail."""
        try:
            cursor.execute(
                "EXEC sp_set_session_context @key = N'heer_node_id', @value = NULL"
            )
            with pytest.raises(pyodbc.ProgrammingError):
                cursor.execute("EXEC generate_id")
        except pyodbc.ProgrammingError:
            pass

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=0, requesting more IDs than fit in one tick fails."""
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ids @in_node_id = 1, "
                "@requested_count = 8193, @allow_spanning = 0"
            )


# ── RanjId: Basic Generation ──


class TestRanjIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 10"
        )
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [
            RanjId.from_str(str(uuid.UUID(bytes=bytes(r[0])))) for r in rows
        ]
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_ranj_node_id @node_id = 1")
        cursor.execute("EXEC generate_ranjid")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1

    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 100"
        )
        rows = cursor.fetchall()
        ids = [bytes(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_sort_correctly(self, cursor):
        """BINARY(16) should sort in the same order as UUID string sort."""
        cursor.execute(
            "EXEC generate_ranjids @in_node_id = 1, @requested_count = 50"
        )
        rows = cursor.fetchall()
        raw_bytes = [bytes(r[0]) for r in rows]
        for i in range(len(raw_bytes) - 1):
            assert raw_bytes[i] < raw_bytes[i + 1]

    def test_ranjid_is_valid_uuidv7(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        assert u.version == 7
        assert (u.int >> 62) & 0b11 == 0b10

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        rid1 = RanjId.from_str(
            str(uuid.UUID(bytes=bytes(cursor.fetchone()[0])))
        )
        cursor.execute("EXEC generate_ranjid @in_node_id = 2")
        rid2 = RanjId.from_str(
            str(uuid.UUID(bytes=bytes(cursor.fetchone()[0])))
        )
        assert rid1.node_id == 1
        assert rid2.node_id == 2

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = set()
        for _ in range(10):
            cursor.execute("EXEC generate_ranjid @in_node_id = 1")
            all_ids.add(bytes(cursor.fetchone()[0]))
        assert len(all_ids) == 10


# ── RanjId: Error Cases ──


class TestRanjIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute("EXEC generate_ranjid @in_node_id = 99999")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, @requested_count = 0"
            )

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, @requested_count = -1"
            )

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=0, requesting more than 65536 RanjIds in one tick fails."""
        with pytest.raises(pyodbc.ProgrammingError):
            cursor.execute(
                "EXEC generate_ranjids @in_node_id = 1, "
                "@requested_count = 65537, @allow_spanning = 0"
            )


# ── Django Fields Against Real MSSQL ──


class TestDjangoFieldsMssql:
    """Test Django field methods using real MSSQL-generated values."""

    def test_heerid_field_from_db_value(self, cursor):
        """HeerIdField.from_db_value works with MSSQL integer results."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]

        field = HeerIdField()
        hid = field.from_db_value(int(raw), None, None)
        assert isinstance(hid, HeerId)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_heerid_field_prep_roundtrip(self, cursor):
        """HeerId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import HeerIdField

        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]
        original = HeerId(int(raw))

        field = HeerIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(prep, None, None)
        assert restored.as_int() == original.as_int()
        assert restored.node_id == original.node_id

    def test_ranjid_field_from_db_value_bytes(self, cursor):
        """RanjIdField.from_db_value works with MSSQL BINARY(16) bytes."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]

        field = RanjIdField()
        rid = field.from_db_value(bytes(raw_bytes), None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_ranjid_field_from_db_value_memoryview(self, cursor):
        """RanjIdField.from_db_value works with memoryview (pyodbc returns this)."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]

        field = RanjIdField()
        mv = memoryview(bytes(raw_bytes))
        rid = field.from_db_value(mv, None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1

    def test_ranjid_field_prep_roundtrip(self, cursor):
        """RanjId survives get_prep_value -> from_db_value roundtrip."""
        from heeranjid_django.fields import RanjIdField

        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        original = RanjId.from_str(str(u))

        field = RanjIdField()
        prep = field.get_prep_value(original)
        assert isinstance(prep, uuid.UUID)
        restored = field.from_db_value(str(prep), None, None)
        assert restored.node_id == original.node_id
        assert restored.sequence == original.sequence

    def test_ranjid_field_db_type_mssql(self, cursor):
        """RanjIdField returns BINARY(16) for MSSQL vendor."""
        from heeranjid_django.fields import RanjIdField

        class _FakeConn:
            vendor = "microsoft"

        field = RanjIdField()
        assert field.db_type(_FakeConn()) == "BINARY(16)"


# ── Concurrency ──


class TestConcurrencyMssql:
    def test_concurrent_heerid_uniqueness(self, mssql_conn):
        """Multiple connections generating HeerId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = pyodbc.connect(MSSQL_URL, autocommit=True)
                cur = conn.cursor()
                cur.execute("USE heeranjid_test")
                cur.execute(
                    "EXEC generate_ids @in_node_id = 1, @requested_count = 50"
                )
                rows = cur.fetchall()
                results.extend([int(r[0]) for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate HeerId detected under concurrency"

    def test_concurrent_ranjid_uniqueness(self, mssql_conn):
        """Multiple connections generating RanjId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = pyodbc.connect(MSSQL_URL, autocommit=True)
                cur = conn.cursor()
                cur.execute("USE heeranjid_test")
                cur.execute(
                    "EXEC generate_ranjids @in_node_id = 1, @requested_count = 50"
                )
                rows = cur.fetchall()
                results.extend([bytes(r[0]) for r in rows])
                cur.close()
                conn.close()
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=generate_ids) for _ in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=30)

        assert not errors, f"Threads raised errors: {errors}"
        assert len(results) == 200
        assert len(set(results)) == 200, "Duplicate RanjId detected under concurrency"
```

- [ ] **Step 4: Delete old test files from core package**

```bash
git rm bindings/python/tests/test_django_fields.py
git rm bindings/python/tests/test_postgres_integration.py
git rm bindings/python/tests/test_mssql_integration.py
```

- [ ] **Step 5: Run Django field tests**

```bash
cd bindings/python/django && pip install -e . && pytest tests/test_django_fields.py -v
```

Expected: all 22 tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move Django tests to heeranjid-django, update imports"
```

---

### Task 5: Remove Django code from Python core and clean up pyproject.toml

Remove the Django-specific code from the core package and clean up the package metadata.

**Files:**
- Delete: `bindings/python/python/heeranjid/django/` (entire directory)
- Modify: `bindings/python/pyproject.toml`

- [ ] **Step 1: Delete Django directory from core**

```bash
git rm -r bindings/python/python/heeranjid/django/
```

- [ ] **Step 2: Update pyproject.toml**

Edit `bindings/python/pyproject.toml` to remove Django references:

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "heeranjid"
version = "0.1.0"
description = "Distributed ID generation — HeerId (64-bit) and RanjId (128-bit UUIDv7)"
requires-python = ">=3.10"
license = "MIT"
classifiers = [
    "Programming Language :: Rust",
    "Programming Language :: Python :: Implementation :: CPython",
]

[project.optional-dependencies]
dev = ["pytest>=8.0", "maturin>=1.0"]

[tool.maturin]
python-source = "python"
module-name = "heeranjid._heeranjid"
features = ["pyo3/extension-module"]
```

Changes:
- Removed `Framework :: Django` classifiers (all 3)
- Removed `django = ["django>=4.2"]` from optional dependencies

- [ ] **Step 3: Verify core tests still pass**

```bash
cd bindings/python && pytest tests/test_heerid.py tests/test_ranjid.py tests/test_sql_constants.py -v
```

Expected: all tests pass. No Django imports anywhere in core.

- [ ] **Step 4: Verify Django package still works**

```bash
cd bindings/python/django && pytest tests/test_django_fields.py -v
```

Expected: all tests pass. The Django package imports from `heeranjid` (core) and `heeranjid_django` (itself).

- [ ] **Step 5: Run the Rust lint checks**

```bash
bash scripts/check.sh
```

Expected: all checks pass (fmt, clippy, deny).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(python): remove Django code from core, clean up pyproject.toml"
```
