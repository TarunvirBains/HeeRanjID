# Language/Framework Separation Design

## Goal

Separate language bindings from framework integrations so that each is an independent package. Framework packages depend on their language package, not the other way around. Third parties can build framework integrations (e.g., `heeranjid-fastapi`, `heeranjid-seaorm`) by depending on the language package and consuming its SQL constants.

## Scope

This spec covers the Python split only. Node, .NET, and Rust follow the same pattern in future PRs. Rust is already correct (`heeranjid` core + `heeranjid-sqlx` framework).

## Problem

The Python package `heeranjid` currently bundles Django-specific code (fields, apps, migrations) alongside the core types and SQL. This means:

- Installing `heeranjid` for use with FastAPI or plain Python still ships Django classifiers and Django-specific modules.
- The import path `heeranjid.django` is ambiguous with the Django framework itself.
- A third-party framework author has no clear template for how to build their own integration — the Django code is interleaved with the core package.
- The pattern doesn't match Rust, where `heeranjid` (core) and `heeranjid-sqlx` (framework) are separate crates.

## Design Principle

**Language packages provide types and raw SQL. Framework packages consume both to integrate with a specific ORM/framework.**

The layering, from bottom to top:

1. **Rust core** (`heeranjid`) — types, encoding/decoding. No SQL, no IO.
2. **Language binding** (`bindings/python/`, `bindings/node/`, etc.) — wraps core types for the language. Exposes SQL as module-level string constants when built with `include-sql`.
3. **Framework integration** (`bindings/python/django/`, etc.) — depends on the language binding. Uses the SQL constants to build migrations. Could be first-party or third-party.

SQL stops at the language layer. Framework authors read it from there.

## Monorepo Layout

```
HeeRanjID/
  sql/                              # git submodule — single source of truth
  heeranjid/                        # Rust core crate
  heeranjid-ffi/                    # C FFI crate
  heeranjid-sqlx/                   # Rust framework (sqlx)
  bindings/
    python/                         # heeranjid (Python language package)
      python/heeranjid/
        __init__.py                 # exports HeerId, RanjId
        _heeranjid.so               # compiled Rust extension
        py.typed
        sql/
          __init__.py               # empty package marker
          postgres/
            __init__.py             # module-level SQL constants
            *.sql                   # gitignored, copied at build time
          mssql/
            __init__.py             # module-level SQL constants
            *.sql                   # gitignored, copied at build time
      src/                          # Rust PyO3 source
      pyproject.toml                # maturin build, no Django dependency
      Makefile                      # copy-sql + maturin
      Cargo.toml
      tests/
        test_heerid.py
        test_ranjid.py
        test_sql_constants.py       # new: verifies SQL constants load
      django/                       # heeranjid-django (framework package)
        src/
          heeranjid_django/
            __init__.py             # exports HeerIdField, RanjIdField
            apps.py                 # Django AppConfig
            fields.py               # HeerIdField, RanjIdField
            migrations/
              __init__.py
              0001_install_heeranjid.py
        pyproject.toml              # hatchling, depends on heeranjid + django>=4.2
        tests/
          test_django_fields.py
          test_postgres_integration.py
          test_mssql_integration.py
    node/                           # heeranjid-node (future refactor)
      prisma/                       # heeranjid-prisma (future)
    dotnet/                         # heeranjid-dotnet (future refactor)
      efcore/                       # heeranjid-efcore (future)
```

Rust crates stay at the repo root since they're part of the Cargo workspace. `heeranjid-sqlx` is already a framework crate at the root, which is consistent.

## SQL Module Design

The `heeranjid.sql` module exposes SQL as module-level string constants, loaded once at import time via `importlib.resources`. This mirrors Rust's `include_str!` pattern.

### `python/heeranjid/sql/postgres/__init__.py`

```python
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
        "SQL files not found. Build with 'make dev' or 'make build' to copy "
        "SQL files from the sql/ submodule."
    )
```

`heeranjid.sql.mssql.__init__.py` follows the same pattern.

The `.sql` files are:
- Copied at build time by the Makefile (`make copy-sql` from `sql/` submodule)
- Gitignored (build artifacts)
- Included in the wheel by maturin (they're inside `python/heeranjid/`)

The `__init__.py` files are tracked in git.

### Usage by framework authors

```python
from heeranjid.sql import postgres

# Access raw SQL strings
postgres.SCHEMA      # CREATE TABLE heer_nodes ...
postgres.SEED        # INSERT INTO heer_nodes ...
postgres.SESSION     # CREATE FUNCTION set_heer_node_id ...
```

## `heeranjid-django` Package

### `pyproject.toml`

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

[tool.hatch.build.targets.wheel]
packages = ["src/heeranjid_django"]
```

### Import path changes

| Before | After |
|--------|-------|
| `from heeranjid.django import HeerIdField` | `from heeranjid_django import HeerIdField` |
| `from heeranjid.django.fields import HeerIdField` | `from heeranjid_django.fields import HeerIdField` |
| `INSTALLED_APPS: "heeranjid.django"` | `INSTALLED_APPS: "heeranjid_django"` |

### Migration SQL loading

Before:
```python
def _read_sql(backend, filename):
    package = f"heeranjid.sql.{backend}"
    return resources.files(package).joinpath(filename).read_text(encoding="utf-8")
```

After:
```python
from heeranjid.sql import postgres, mssql

def _get_sql_module(schema_editor):
    if schema_editor.connection.vendor == "microsoft":
        return mssql
    return postgres
```

Then access `sql_module.SCHEMA`, `sql_module.SESSION`, etc.

### `deconstruct()` paths

Both `HeerIdField` and `RanjIdField` update their `deconstruct()` to return `"heeranjid_django.fields.HeerIdField"` and `"heeranjid_django.fields.RanjIdField"`.

### `apps.py`

```python
from django.apps import AppConfig

class HeeranjidConfig(AppConfig):
    name = "heeranjid_django"
    verbose_name = "HeeRanjID"
    default_auto_field = "django.db.models.BigAutoField"
```

## Changes to `heeranjid` (Python core)

- Remove `python/heeranjid/django/` directory entirely
- Remove `django` from `[project.optional-dependencies]`
- Remove Django classifiers from `pyproject.toml`
- Add constants-based `__init__.py` to `sql/postgres/` and `sql/mssql/` (replacing empty markers)
- Update Makefile `SQL_SRC` path from `../sql` to `../../sql` (one level deeper under `bindings/`)
- Update Cargo workspace `members` in root `Cargo.toml` (`heeranjid-python` becomes `bindings/python`)
- Update CI workflow if it references `heeranjid-python/` by path

## Testing

**Core package tests (`bindings/python/tests/`):**
- `test_heerid.py` — unchanged
- `test_ranjid.py` — unchanged
- `test_sql_constants.py` — new, verifies `heeranjid.sql.postgres.SCHEMA` etc. are non-empty strings

**Django package tests (`bindings/python/django/tests/`):**
- `test_django_fields.py` — moved, imports updated to `heeranjid_django`
- `test_postgres_integration.py` — moved, imports updated
- `test_mssql_integration.py` — moved, imports updated

No new test logic. This is a packaging/import change, not a behavior change.

## What This Enables

A third-party author building `heeranjid-fastapi` would:

1. `pip install heeranjid` (with SQL)
2. `from heeranjid.sql import postgres`
3. Use `postgres.SCHEMA`, `postgres.GENERATE_HEERID`, etc. to set up the database through whatever mechanism FastAPI/SQLAlchemy provides
4. Use `from heeranjid import HeerId, RanjId` for the types

They never touch the `sql/` submodule. They don't need maturin. They just depend on the built wheel.

## Future Work (Out of Scope)

- Node: split `heeranjid-node` into `bindings/node/` + `bindings/node/prisma/`
- .NET: split EF Core converters into `bindings/dotnet/efcore/`
- Rust: already correct, no changes needed
- Documentation: "How to build a framework integration" guide
