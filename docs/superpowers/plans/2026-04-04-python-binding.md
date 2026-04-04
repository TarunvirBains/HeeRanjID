# Python Binding (heeranjid-python) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a PyO3-based Python package (`heeranjid`) that wraps the core Rust types (HeerId, RanjId) and provides Django model fields with automatic Postgres schema migration.

**Architecture:** PyO3 compiles the Rust core directly into a Python native extension. The native module exposes `HeerId` and `RanjId` as Python classes with read-only properties for decoded fields. A pure-Python layer provides Django integration (custom fields, migration). No ID generation in Rust — Django models use `dbgenerated` defaults that call Postgres functions directly.

**Tech Stack:** PyO3, maturin, Python >= 3.10, Django >= 4.2 (optional), pytest

**Prerequisites:** Workspace restructure (Task 1-3 from workspace-restructure plan) must be complete.

---

### Task 1: Scaffold the Python binding crate

**Files:**
- Create: `heeranjid-python/Cargo.toml`
- Create: `heeranjid-python/pyproject.toml`
- Create: `heeranjid-python/src/lib.rs`
- Create: `heeranjid-python/python/heeranjid/__init__.py`
- Create: `heeranjid-python/python/heeranjid/py.typed`
- Modify: root `Cargo.toml` (add to workspace members)

- [ ] **Step 1: Add heeranjid-python to workspace members**

In root `Cargo.toml`, change:

```toml
[workspace]
members = ["heeranjid"]
resolver = "2"
```

to:

```toml
[workspace]
members = ["heeranjid", "heeranjid-python"]
resolver = "2"
```

- [ ] **Step 2: Create heeranjid-python/Cargo.toml**

```toml
[package]
name = "heeranjid-python"
version = "0.1.0"
edition = "2024"
license = "MIT"
publish = false

[lib]
name = "_heeranjid"
crate-type = ["cdylib"]

[dependencies]
heeranjid = { path = "../heeranjid", default-features = false }
pyo3 = { version = "0.24", features = ["extension-module"] }
uuid = "1"
```

- [ ] **Step 3: Create pyproject.toml**

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
    "Framework :: Django",
    "Framework :: Django :: 4.2",
    "Framework :: Django :: 5.2",
]

[project.optional-dependencies]
django = ["django>=4.2"]
dev = ["pytest>=8.0", "maturin>=1.0"]

[tool.maturin]
python-source = "python"
module-name = "heeranjid._heeranjid"
features = ["pyo3/extension-module"]
```

- [ ] **Step 4: Create minimal src/lib.rs**

```rust
use pyo3::prelude::*;

#[pymodule]
fn _heeranjid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
```

- [ ] **Step 5: Create python/heeranjid/__init__.py**

```python
"""HeeRanjID — distributed ID generation types."""

from heeranjid._heeranjid import HeerId, RanjId

__all__ = ["HeerId", "RanjId"]
```

- [ ] **Step 6: Create python/heeranjid/py.typed**

Empty file (PEP 561 marker):

```
```

- [ ] **Step 7: Verify the scaffold builds**

Run: `cd heeranjid-python && maturin develop`
Expected: SUCCESS — installs the package into the current Python environment

Run: `python -c "import heeranjid"` 
Expected: ImportError about HeerId/RanjId not existing yet (that's fine — we haven't implemented them)

- [ ] **Step 8: Commit**

```bash
git add heeranjid-python/ Cargo.toml
git commit -m "feat: scaffold heeranjid-python crate with PyO3 and maturin"
```

---

### Task 2: Implement HeerId Python wrapper

**Files:**
- Modify: `heeranjid-python/src/lib.rs`
- Create: `heeranjid-python/tests/test_heerid.py`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-python/tests/test_heerid.py`:

```python
import pytest
from heeranjid import HeerId


def test_heerid_from_int():
    hid = HeerId(0)
    assert hid.as_int() == 0


def test_heerid_rejects_negative():
    with pytest.raises(ValueError, match="non-negative"):
        HeerId(-1)


def test_heerid_decodes_parts():
    # Build a known ID: timestamp=1000, node=5, sequence=42
    # Bit layout: timestamp(41) | node(9) | sequence(13)
    raw = (1000 << 22) | (5 << 13) | 42
    hid = HeerId(raw)
    assert hid.timestamp_ms == 1000
    assert hid.node_id == 5
    assert hid.sequence == 42


def test_heerid_str():
    hid = HeerId(12345)
    assert str(hid) == "12345"


def test_heerid_repr():
    hid = HeerId(12345)
    assert repr(hid) == "HeerId(12345)"


def test_heerid_equality():
    a = HeerId(100)
    b = HeerId(100)
    c = HeerId(200)
    assert a == b
    assert a != c


def test_heerid_ordering():
    a = HeerId(100)
    b = HeerId(200)
    assert a < b
    assert b > a


def test_heerid_hash():
    a = HeerId(100)
    b = HeerId(100)
    assert hash(a) == hash(b)
    s = {a, b}
    assert len(s) == 1


def test_heerid_from_str():
    hid = HeerId.from_str("12345")
    assert hid.as_int() == 12345


def test_heerid_from_str_rejects_garbage():
    with pytest.raises(ValueError):
        HeerId.from_str("not_a_number")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-python && maturin develop && pytest tests/test_heerid.py -v`
Expected: FAIL — HeerId not yet exposed from the native module

- [ ] **Step 3: Implement HeerId wrapper**

Replace `heeranjid-python/src/lib.rs` with:

```rust
use pyo3::prelude::*;

#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeerId {
    inner: heeranjid::HeerId,
}

#[pymethods]
impl HeerId {
    #[new]
    fn py_new(value: i64) -> PyResult<Self> {
        let inner = heeranjid::HeerId::from_i64(value)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::HeerId = s
            .parse()
            .map_err(|e: heeranjid::Error| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn as_int(&self) -> i64 {
        self.inner.as_i64()
    }

    #[getter]
    fn timestamp_ms(&self) -> u64 {
        self.inner.timestamp_ms()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("HeerId({})", self.inner.as_i64())
    }
}

#[pymodule]
fn _heeranjid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HeerId>()?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd heeranjid-python && maturin develop && pytest tests/test_heerid.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add heeranjid-python/src/lib.rs heeranjid-python/tests/test_heerid.py
git commit -m "feat(python): implement HeerId wrapper with PyO3"
```

---

### Task 3: Implement RanjId Python wrapper

**Files:**
- Modify: `heeranjid-python/src/lib.rs`
- Create: `heeranjid-python/tests/test_ranjid.py`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-python/tests/test_ranjid.py`:

```python
import uuid
import pytest
from heeranjid import RanjId


def test_ranjid_from_str():
    # Build a known RanjId: timestamp=1_000_000, node=100, sequence=200
    # Use the Rust core to construct a valid UUIDv7 string for this
    rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    assert isinstance(rid, RanjId)


def test_ranjid_rejects_non_v7():
    # UUID v4 (random) should be rejected
    with pytest.raises(ValueError, match="version"):
        RanjId.from_str("550e8400-e29b-41d4-a716-446655440000")


def test_ranjid_decodes_parts():
    rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    assert rid.timestamp_micros == 1_000_000
    assert rid.node_id == 100
    assert rid.sequence == 200


def test_ranjid_to_uuid():
    rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    u = rid.to_uuid()
    assert isinstance(u, uuid.UUID)
    assert u.version == 7


def test_ranjid_str():
    rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    s = str(rid)
    assert s == "00000000-0f42-7040-8000-006400c8"


def test_ranjid_repr():
    rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    assert repr(rid).startswith("RanjId(")


def test_ranjid_equality():
    a = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    b = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    assert a == b


def test_ranjid_hash():
    a = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    b = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
    assert hash(a) == hash(b)


def test_ranjid_from_str_rejects_garbage():
    with pytest.raises(ValueError):
        RanjId.from_str("not-a-uuid")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-python && maturin develop && pytest tests/test_ranjid.py -v`
Expected: FAIL — RanjId not yet exposed

- [ ] **Step 3: Implement RanjId wrapper**

In `heeranjid-python/src/lib.rs`, add above the `_heeranjid` module function:

```rust
#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RanjId {
    inner: heeranjid::RanjId,
}

#[pymethods]
impl RanjId {
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::RanjId = s
            .parse()
            .map_err(|e: heeranjid::Error| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn to_uuid(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let uuid_mod = py.import("uuid")?;
            let uuid_class = uuid_mod.getattr("UUID")?;
            let uuid_str = self.inner.as_uuid().to_string();
            let result = uuid_class.call1((uuid_str,))?;
            Ok(result.into())
        })
    }

    #[getter]
    fn timestamp_micros(&self) -> u128 {
        self.inner.timestamp_micros()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("RanjId({})", self.inner.as_uuid())
    }
}
```

And update the module registration:

```rust
#[pymodule]
fn _heeranjid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HeerId>()?;
    m.add_class::<RanjId>()?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd heeranjid-python && maturin develop && pytest tests/test_ranjid.py tests/test_heerid.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add heeranjid-python/src/lib.rs heeranjid-python/tests/test_ranjid.py
git commit -m "feat(python): implement RanjId wrapper with PyO3"
```

---

### Task 4: Django model fields

**Files:**
- Create: `heeranjid-python/python/heeranjid/django/__init__.py`
- Create: `heeranjid-python/python/heeranjid/django/fields.py`
- Create: `heeranjid-python/tests/test_django_fields.py`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-python/tests/test_django_fields.py`:

```python
"""Tests for Django field types.

These tests verify field behavior without a running database.
They test serialization, deserialization, and field configuration.
"""
import uuid
import pytest

django = pytest.importorskip("django")

import django.conf
django.conf.settings.configure(
    DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
    INSTALLED_APPS=["django.contrib.contenttypes"],
    DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
)
import django as dj
dj.setup()

from heeranjid.django import HeerIdField, RanjIdField
from heeranjid import HeerId, RanjId


class TestHeerIdField:
    def setup_method(self):
        self.field = HeerIdField()

    def test_internal_type(self):
        assert self.field.get_internal_type() == "BigIntegerField"

    def test_from_db_value_none(self):
        assert self.field.from_db_value(None, None, None) is None

    def test_from_db_value_int(self):
        result = self.field.from_db_value(12345, None, None)
        assert isinstance(result, HeerId)
        assert result.as_int() == 12345

    def test_get_prep_value_none(self):
        assert self.field.get_prep_value(None) is None

    def test_get_prep_value_heerid(self):
        hid = HeerId(12345)
        assert self.field.get_prep_value(hid) == 12345

    def test_get_prep_value_int(self):
        assert self.field.get_prep_value(12345) == 12345

    def test_db_default(self):
        field = HeerIdField(primary_key=True)
        # The field should set db_default to call the Postgres function
        assert field.db_default is not None


class TestRanjIdField:
    def setup_method(self):
        self.field = RanjIdField()

    def test_internal_type(self):
        assert self.field.get_internal_type() == "UUIDField"

    def test_from_db_value_none(self):
        assert self.field.from_db_value(None, None, None) is None

    def test_from_db_value_uuid(self):
        # Create a valid UUIDv7 to parse
        rid_orig = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
        u = rid_orig.to_uuid()
        result = self.field.from_db_value(u, None, None)
        assert isinstance(result, RanjId)

    def test_get_prep_value_none(self):
        assert self.field.get_prep_value(None) is None

    def test_get_prep_value_ranjid(self):
        rid = RanjId.from_str("00000000-0f42-7040-8000-006400c8")
        result = self.field.get_prep_value(rid)
        assert isinstance(result, uuid.UUID)

    def test_db_default(self):
        field = RanjIdField(primary_key=True)
        assert field.db_default is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-python && maturin develop && pip install django>=4.2 && pytest tests/test_django_fields.py -v`
Expected: FAIL — module `heeranjid.django` does not exist or has no fields

- [ ] **Step 3: Implement Django fields**

Create `heeranjid-python/python/heeranjid/django/__init__.py`:

```python
from heeranjid.django.fields import HeerIdField, RanjIdField

__all__ = ["HeerIdField", "RanjIdField"]
```

Create `heeranjid-python/python/heeranjid/django/fields.py`:

```python
from django.db import models
from django.db.models.expressions import RawSQL

from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    """A Django model field that stores a HeerId as a BIGINT.

    When used as a primary key, automatically sets the database default
    to call the Postgres heer_generate_id() function.
    """

    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("heer_generate_id()", [])
        super().__init__(*args, **kwargs)

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
        return name, "heeranjid.django.fields.HeerIdField", args, kwargs


class RanjIdField(models.UUIDField):
    """A Django model field that stores a RanjId as a UUID.

    When used as a primary key, automatically sets the database default
    to call the Postgres ranj_generate_id() function.
    """

    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("ranj_generate_id()", [])
        super().__init__(*args, **kwargs)

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
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
        return name, "heeranjid.django.fields.RanjIdField", args, kwargs
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd heeranjid-python && maturin develop && pytest tests/test_django_fields.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add heeranjid-python/python/heeranjid/django/ heeranjid-python/tests/test_django_fields.py
git commit -m "feat(python): add Django HeerIdField and RanjIdField"
```

---

### Task 5: Django SQL migration

**Files:**
- Create: `heeranjid-python/python/heeranjid/django/migrations/__init__.py`
- Create: `heeranjid-python/python/heeranjid/django/migrations/0001_install_heeranjid.py`

- [ ] **Step 1: Create the migrations package**

Create `heeranjid-python/python/heeranjid/django/migrations/__init__.py`:

```python
```

- [ ] **Step 2: Create the SQL migration**

The migration reads SQL from the bundled files and installs the schema and functions.

Create `heeranjid-python/python/heeranjid/django/migrations/0001_install_heeranjid.py`:

```python
"""Install HeeRanjID Postgres schema and functions.

This migration installs the heer_nodes, heer_config, heer_node_state,
and heer_ranj_node_state tables, plus the generate_id(), generate_ids(),
generate_ranjid(), generate_ranjids(), and session functions.
"""
from importlib.resources import files
from django.db import migrations


def get_install_sql():
    """Read the bundled SQL install script."""
    sql_dir = files("heeranjid") / "sql"
    parts = []
    for filename in [
        "schema.sql",
        "session.sql",
        "generate_heerid.sql",
        "generate_ranjid.sql",
    ]:
        parts.append((sql_dir / filename).read_text(encoding="utf-8"))
    return "\n".join(parts)


class Migration(migrations.Migration):
    initial = True
    dependencies = []

    operations = [
        migrations.RunSQL(
            sql=get_install_sql(),
            reverse_sql="DROP FUNCTION IF EXISTS generate_id(integer) CASCADE; "
                        "DROP FUNCTION IF EXISTS generate_ids(integer, integer) CASCADE; "
                        "DROP FUNCTION IF EXISTS generate_ranjid(integer) CASCADE; "
                        "DROP FUNCTION IF EXISTS generate_ranjids(integer, integer) CASCADE; "
                        "DROP FUNCTION IF EXISTS set_heer_ranj_node_id(integer) CASCADE; "
                        "DROP FUNCTION IF EXISTS heer_start_session(integer) CASCADE; "
                        "DROP TABLE IF EXISTS heer_ranj_node_state CASCADE; "
                        "DROP TABLE IF EXISTS heer_node_state CASCADE; "
                        "DROP TABLE IF EXISTS heer_nodes CASCADE; "
                        "DROP TABLE IF EXISTS heer_config CASCADE;",
        ),
    ]
```

- [ ] **Step 3: Bundle the SQL files into the Python package**

Create `heeranjid-python/python/heeranjid/sql/` directory with symlinks or copies of the SQL files. Since the sql submodule is at the repo root, we'll copy them during build. Add a build script approach — or simpler, just copy the files directly.

Create the sql resource directory and copy files:

```bash
mkdir -p heeranjid-python/python/heeranjid/sql
cp sql/postgres/schema.sql heeranjid-python/python/heeranjid/sql/
cp sql/postgres/functions/session.sql heeranjid-python/python/heeranjid/sql/
cp sql/postgres/functions/generate_heerid.sql heeranjid-python/python/heeranjid/sql/
cp sql/postgres/functions/generate_ranjid.sql heeranjid-python/python/heeranjid/sql/
cp sql/postgres/seed.sql heeranjid-python/python/heeranjid/sql/
```

Note: These are checked-in copies. When the sql submodule updates, these need to be re-copied. A future CI step can automate this verification.

- [ ] **Step 4: Verify migration loads without error**

Run: `cd heeranjid-python && maturin develop && python -c "from heeranjid.django.migrations.install_heeranjid_0001 import Migration; print('OK')"`

Note: This only verifies the migration class loads. Actually running `migrate` requires a Postgres database with the heeranjid functions, which is an integration test concern.

- [ ] **Step 5: Commit**

```bash
git add heeranjid-python/python/heeranjid/sql/ heeranjid-python/python/heeranjid/django/migrations/
git commit -m "feat(python): add Django migration to install HeeRanjID Postgres schema"
```

---

### Task 6: Add the Django app configuration

**Files:**
- Create: `heeranjid-python/python/heeranjid/django/apps.py`
- Modify: `heeranjid-python/python/heeranjid/django/__init__.py`

- [ ] **Step 1: Create the Django app config**

Create `heeranjid-python/python/heeranjid/django/apps.py`:

```python
from django.apps import AppConfig


class HeeranjidConfig(AppConfig):
    name = "heeranjid.django"
    label = "heeranjid"
    verbose_name = "HeeRanjID"
    default_auto_field = "django.db.models.BigAutoField"
```

- [ ] **Step 2: Set default_app_config in django/__init__.py**

Update `heeranjid-python/python/heeranjid/django/__init__.py`:

```python
from heeranjid.django.fields import HeerIdField, RanjIdField

default_app_config = "heeranjid.django.apps.HeeranjidConfig"

__all__ = ["HeerIdField", "RanjIdField"]
```

- [ ] **Step 3: Commit**

```bash
git add heeranjid-python/python/heeranjid/django/
git commit -m "feat(python): add Django app config for heeranjid"
```

---

### Task 7: Final verification and cleanup

**Files:**
- No new files

- [ ] **Step 1: Run all Python tests**

Run: `cd heeranjid-python && maturin develop && pytest tests/ -v`
Expected: All tests PASS

- [ ] **Step 2: Run Rust workspace checks**

Run: `cargo clippy --workspace -- -D warnings`
Expected: SUCCESS

Run: `cargo fmt --all --check`
Expected: SUCCESS

Run: `cargo test -p heeranjid --lib`
Expected: All unit tests PASS

- [ ] **Step 3: Verify the package installs cleanly**

Run: `cd heeranjid-python && maturin build --release`
Expected: Wheel file produced in `target/wheels/`

Run: `pip install target/wheels/heeranjid-*.whl --force-reinstall && python -c "from heeranjid import HeerId, RanjId; print(HeerId(42)); print(RanjId.from_str('00000000-0f42-7040-8000-006400c8'))"`
Expected: Prints `42` and the UUID string

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore(python): final cleanup for heeranjid-python package"
```
