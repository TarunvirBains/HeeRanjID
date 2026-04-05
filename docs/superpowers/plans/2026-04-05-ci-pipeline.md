# CI Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Multi-language CI pipeline where the Rust job builds all binding artifacts once, then fans out to Python, TypeScript, and .NET test jobs.

**Architecture:** Single workflow file with 4 jobs. Rust job (Bookworm) lints, tests, and builds Python wheel + TypeScript .node + FFI .so artifacts. Language jobs download their artifact and run tests. Postgres test parity is enforced before CI goes live.

**Tech Stack:** GitHub Actions, Rust 1.94, Python 3.11, Node 18, .NET SDK 8, maturin, napi-rs, Postgres

---

## File Structure

### Files to modify

| File | Change |
|------|--------|
| `.github/workflows/ci.yml` | Rewrite: Bookworm image, artifact builds, 3 new language jobs |

### Files to create

| File | Purpose |
|------|---------|
| `bindings/python/django/tests/test_postgres_integration.py` | Expand Postgres tests to MSSQL parity |

### Files unchanged (verified by CI)

| File | Role in CI |
|------|-----------|
| `bindings/python/tests/test_heerid.py` | Core type tests (Python job) |
| `bindings/python/tests/test_ranjid.py` | Core type tests (Python job) |
| `bindings/python/tests/test_sql_constants.py` | SQL constants tests (Python job) |
| `bindings/python/django/tests/test_django_fields.py` | Django field unit tests (Python job) |
| `bindings/typescript/tests/heerid.test.ts` | HeerId type tests (TypeScript job) |
| `bindings/typescript/tests/ranjid.test.ts` | RanjId type tests (TypeScript job) |
| `bindings/typescript/tests/prisma.test.ts` | Prisma extension shape tests (TypeScript job) |
| `bindings/dotnet/tests/HeeRanjID.Tests/` | .NET type tests (.NET job) |

---

### Task 1: Bring Postgres integration tests to parity with MSSQL

The current Postgres test file has 6 tests. The MSSQL file has 33. Add the missing tests to Postgres so both backends have identical coverage.

**Files:**
- Modify: `bindings/python/django/tests/test_postgres_integration.py`

- [ ] **Step 1: Read the current Postgres and MSSQL test files**

Read `bindings/python/django/tests/test_postgres_integration.py` and `bindings/python/django/tests/test_mssql_integration.py` to understand what's missing.

- [ ] **Step 2: Register node 2 in the Postgres fixture**

The MSSQL fixture registers node 2 for multi-node tests. Add the same to the Postgres `pg_conn` fixture. After the seed SQL execution, add:

```python
    cur.execute("""
        INSERT INTO heer_nodes (node_id, name, description, is_active)
        VALUES (2, 'test-node-2', 'Second test node', true)
        ON CONFLICT (node_id) DO NOTHING
    """)
```

- [ ] **Step 3: Add missing HeerId tests to TestHeerIdPostgres**

Add these tests to the existing `TestHeerIdPostgres` class:

```python
    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 100)")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_monotonically_increasing(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 50)")
        rows = cursor.fetchall()
        ids = [int(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = []
        for _ in range(5):
            cursor.execute("SELECT generate_id(1)")
            all_ids.append(int(cursor.fetchone()[0]))
        assert len(set(all_ids)) == 5

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("SELECT generate_id(1)")
        id1 = HeerId(int(cursor.fetchone()[0]))
        cursor.execute("SELECT generate_id(2)")
        id2 = HeerId(int(cursor.fetchone()[0]))
        assert id1.node_id == 1
        assert id2.node_id == 2
        assert id1.as_int() != id2.as_int()

    def test_node_id_roundtrips_through_decode(self, cursor):
        for node in [1, 2]:
            cursor.execute(f"SELECT generate_id({node})")
            hid = HeerId(int(cursor.fetchone()[0]))
            assert hid.node_id == node
```

- [ ] **Step 4: Add HeerId error tests**

Add a new class `TestHeerIdErrors` after `TestHeerIdPostgres`:

```python
class TestHeerIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_id(9999)")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ids(1, 0)")

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ids(1, -1)")

    def test_session_node_id_without_set_fails(self, cursor):
        """A fresh connection without set_heer_node_id should fail."""
        cursor.execute("SELECT set_config('heer.node_id', '', false)")
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_id()")

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        """With allow_spanning=false, requesting more IDs than fit in one tick fails."""
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ids(1, 8193, false)")
```

Note: Postgres raises `psycopg2.errors.RaiseException` for user-thrown errors, while MSSQL raises `pyodbc.ProgrammingError`. The test structure is the same, only the exception type differs. Each error test needs a fresh transaction since Postgres aborts the current transaction on error — add `pg_conn.rollback()` in the cursor fixture or use `autocommit=True`.

- [ ] **Step 5: Add missing RanjId tests to TestRanjIdPostgres**

Add these tests to the existing `TestRanjIdPostgres` class:

```python
    def test_bulk_ids_are_unique(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 100)")
        rows = cursor.fetchall()
        ids = [str(r[0]) for r in rows]
        assert len(set(ids)) == 100

    def test_bulk_ids_sort_correctly(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 50)")
        rows = cursor.fetchall()
        ids = [str(r[0]) for r in rows]
        for i in range(len(ids) - 1):
            assert ids[i] < ids[i + 1]

    def test_ranjid_is_valid_uuidv7(self, cursor):
        import uuid as uuid_mod
        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        u = uuid_mod.UUID(str(raw))
        assert u.version == 7
        assert (u.int >> 62) & 0b11 == 0b10

    def test_different_nodes_produce_different_ids(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        rid1 = RanjId.from_str(str(cursor.fetchone()[0]))
        cursor.execute("SELECT generate_ranjid(2)")
        rid2 = RanjId.from_str(str(cursor.fetchone()[0]))
        assert rid1.node_id == 1
        assert rid2.node_id == 2

    def test_ids_across_calls_are_unique(self, cursor):
        all_ids = set()
        for _ in range(10):
            cursor.execute("SELECT generate_ranjid(1)")
            all_ids.add(str(cursor.fetchone()[0]))
        assert len(all_ids) == 10
```

- [ ] **Step 6: Add RanjId error tests**

Add a new class `TestRanjIdErrors`:

```python
class TestRanjIdErrors:
    def test_invalid_node_id_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjid(99999)")

    def test_zero_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjids(1, 0)")

    def test_negative_count_rejected(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjids(1, -1)")

    def test_allow_spanning_false_rejects_overflow(self, cursor):
        with pytest.raises(psycopg2.errors.RaiseException):
            cursor.execute("SELECT generate_ranjids(1, 65537, false)")
```

- [ ] **Step 7: Add Django field tests against real Postgres**

Add a new class `TestDjangoFieldsPostgres` (mirrors `TestDjangoFieldsMssql`):

```python
class TestDjangoFieldsPostgres:
    """Test Django field methods using real Postgres-generated values."""

    def test_heerid_field_from_db_value(self, cursor):
        from heeranjid_django.fields import HeerIdField

        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]

        field = HeerIdField()
        hid = field.from_db_value(int(raw), None, None)
        assert isinstance(hid, HeerId)
        assert hid.node_id == 1
        assert hid.timestamp_ms > 0

    def test_heerid_field_prep_roundtrip(self, cursor):
        from heeranjid_django.fields import HeerIdField

        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]
        original = HeerId(int(raw))

        field = HeerIdField()
        prep = field.get_prep_value(original)
        restored = field.from_db_value(prep, None, None)
        assert restored.as_int() == original.as_int()
        assert restored.node_id == original.node_id

    def test_ranjid_field_from_db_value(self, cursor):
        from heeranjid_django.fields import RanjIdField

        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]

        field = RanjIdField()
        rid = field.from_db_value(str(raw), None, None)
        assert isinstance(rid, RanjId)
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_ranjid_field_prep_roundtrip(self, cursor):
        import uuid as uuid_mod
        from heeranjid_django.fields import RanjIdField

        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        original = RanjId.from_str(str(raw))

        field = RanjIdField()
        prep = field.get_prep_value(original)
        assert isinstance(prep, uuid_mod.UUID)
        restored = field.from_db_value(str(prep), None, None)
        assert restored.node_id == original.node_id
        assert restored.sequence == original.sequence

    def test_ranjid_field_db_type_postgres(self, cursor):
        from heeranjid_django.fields import RanjIdField

        class _FakeConn:
            vendor = "postgresql"

        field = RanjIdField()
        assert field.db_type(_FakeConn()) == "uuid"
```

- [ ] **Step 8: Add concurrency tests**

Add a new class `TestConcurrencyPostgres`:

```python
class TestConcurrencyPostgres:
    def test_concurrent_heerid_uniqueness(self, pg_conn):
        """Multiple connections generating HeerId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = psycopg2.connect(DATABASE_URL)
                conn.autocommit = True
                cur = conn.cursor()
                cur.execute("SELECT id FROM generate_ids(1, 50)")
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

    def test_concurrent_ranjid_uniqueness(self, pg_conn):
        """Multiple connections generating RanjId simultaneously produce unique IDs."""
        import threading

        results = []
        errors = []

        def generate_ids():
            try:
                conn = psycopg2.connect(DATABASE_URL)
                conn.autocommit = True
                cur = conn.cursor()
                cur.execute("SELECT id FROM generate_ranjids(1, 50)")
                rows = cur.fetchall()
                results.extend([str(r[0]) for r in rows])
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

- [ ] **Step 9: Run tests locally against Postgres**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid \
  /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest \
  bindings/python/django/tests/test_postgres_integration.py -v
```

Expected: all tests pass (should be ~33 tests, matching MSSQL count).

- [ ] **Step 10: Commit**

```bash
git add bindings/python/django/tests/test_postgres_integration.py
git commit -m "test: bring Postgres integration tests to parity with MSSQL"
```

---

### Task 2: Rewrite CI workflow with multi-language jobs

Replace the current single-job workflow with a 4-job pipeline.

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the complete CI workflow**

Replace `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  rust:
    name: Rust (lint, test, build artifacts)
    runs-on: ubuntu-latest
    container:
      image: rust:1.94-slim-bookworm

    services:
      postgres:
        image: postgres:latest
        env:
          POSTGRES_DB: heeranjid
          POSTGRES_USER: postgres
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd "pg_isready -U postgres"
          --health-interval 5s
          --health-timeout 5s
          --health-retries 10

    steps:
      - name: Install system dependencies
        run: |
          apt-get update && apt-get install -y --no-install-recommends \
            git curl python3 python3-pip python3-venv nodejs npm

      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - name: Install Rust tools
        run: |
          rustup component add rustfmt clippy
          DENY_VERSION=$(curl -sL https://api.github.com/repos/EmbarkStudios/cargo-deny/releases/latest | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4)
          curl -sL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${DENY_VERSION}/cargo-deny-${DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
            | tar xz -C /usr/local/bin --strip-components=1 --wildcards '*/cargo-deny'

      - name: Install build tools
        run: |
          pip install maturin --break-system-packages
          npm install -g @napi-rs/cli

      - name: Cache cargo registry & build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: cargo-${{ hashFiles('Cargo.lock') }}
          restore-keys: cargo-

      # Lint
      - name: cargo fmt --check
        run: cargo fmt --all --check

      - name: cargo clippy
        run: cargo clippy --workspace --exclude heeranjid-python --exclude heeranjid-node -- -D warnings

      - name: cargo deny check
        run: cargo deny check

      # Test
      - name: cargo test (unit)
        run: cargo test -p heeranjid --lib

      - name: cargo test (integration)
        run: cargo test -p heeranjid-sqlx --test postgres
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid

      - name: cargo test (concurrency)
        run: cargo test -p heeranjid-sqlx --test concurrency
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid

      # Build binding artifacts
      - name: Build Python wheel
        run: cd bindings/python && maturin build --release
        env:
          PYO3_PYTHON: python3

      - name: Build TypeScript native module
        run: cd bindings/typescript && npm install && npm run build

      - name: Build FFI shared library
        run: cargo build -p heeranjid-ffi --release

      # Upload artifacts
      - name: Upload Python wheel
        uses: actions/upload-artifact@v4
        with:
          name: python-wheel
          path: target/wheels/*.whl

      - name: Upload TypeScript native module
        uses: actions/upload-artifact@v4
        with:
          name: typescript-native
          path: bindings/typescript/heeranjid.*.node

      - name: Upload FFI artifacts
        uses: actions/upload-artifact@v4
        with:
          name: ffi-linux-x64
          path: |
            target/release/libheeranjid_ffi.so
            heeranjid-ffi/heeranjid.h

  python:
    name: Python tests
    needs: rust
    runs-on: ubuntu-latest
    container:
      image: python:3.11-slim-bookworm

    services:
      postgres:
        image: postgres:latest
        env:
          POSTGRES_DB: heeranjid
          POSTGRES_USER: postgres
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd "pg_isready -U postgres"
          --health-interval 5s
          --health-timeout 5s
          --health-retries 10

    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - name: Download Python wheel
        uses: actions/download-artifact@v4
        with:
          name: python-wheel
          path: dist/

      - name: Install packages
        run: |
          pip install dist/*.whl
          pip install -e bindings/python/django/
          pip install pytest django psycopg2-binary

      - name: Run core tests
        run: pytest bindings/python/tests/ -v

      - name: Run Django field tests
        run: pytest bindings/python/django/tests/test_django_fields.py -v

      - name: Run Postgres integration tests
        run: pytest bindings/python/django/tests/test_postgres_integration.py -v
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid

  typescript:
    name: TypeScript tests
    needs: rust
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 18

      - name: Download native module
        uses: actions/download-artifact@v4
        with:
          name: typescript-native
          path: bindings/typescript/

      - name: Install and test
        run: |
          cd bindings/typescript
          npm install
          npm test

  dotnet:
    name: .NET tests
    needs: rust
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - name: Setup .NET
        uses: actions/setup-dotnet@v4
        with:
          dotnet-version: 8.0

      - name: Download FFI artifacts
        uses: actions/download-artifact@v4
        with:
          name: ffi-linux-x64
          path: ffi-artifacts/

      - name: Run tests
        run: dotnet test bindings/dotnet/tests/HeeRanjID.Tests/
        env:
          LD_LIBRARY_PATH: ${{ github.workspace }}/ffi-artifacts/target/release
```

- [ ] **Step 2: Verify the workflow YAML is valid**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" 2>&1 || echo "Invalid YAML"
```

If `pyyaml` isn't installed, visually verify indentation is correct.

- [ ] **Step 3: Run local lint checks**

```bash
bash scripts/check.sh
```

Expected: all checks pass (fmt, clippy, deny).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: multi-language pipeline with Rust artifact fan-out"
```

- [ ] **Step 5: Push and verify CI**

```bash
git push
```

Monitor the GitHub Actions run. Expected:
- `rust` job: lint passes, 24 unit tests, Postgres integration + concurrency, all 3 artifacts uploaded
- `python` job: 31 core tests + 21 Django field tests + ~33 Postgres integration tests
- `typescript` job: HeerId + RanjId + Prisma shape tests
- `dotnet` job: HeerId + RanjId type tests + SqlHelper tests

If any job fails, debug and fix in follow-up commits.

---

### Task 3: Fix issues from CI run

This task is a buffer for debugging CI failures. Common issues to watch for:

**Python job:**
- Wheel architecture mismatch (maturin builds for the container's arch, Python job must match)
- Missing `psycopg2-binary` or `django` in pip install
- SQL constants not loading (submodule not checked out)

**TypeScript job:**
- `.node` file name mismatch (napi-rs names files by platform triple)
- `vitest` not finding tests (working directory wrong)

**Dotnet job:**
- `LD_LIBRARY_PATH` not finding `libheeranjid_ffi.so` (path from download-artifact may differ)
- `heeranjid.h` not needed at test time (only at compile time, and the tests reference the project which has P/Invoke)

**Files:**
- Modify: `.github/workflows/ci.yml` (as needed)

- [ ] **Step 1: Check CI results**

```bash
gh pr checks <PR_NUMBER>
```

Or:

```bash
gh run list --limit 1
gh run view <RUN_ID> --log-failed
```

- [ ] **Step 2: Fix any failures**

Apply targeted fixes based on error output. Common fixes:

- Adjust artifact paths in upload/download steps
- Add missing dependencies
- Fix working directory for test commands

- [ ] **Step 3: Commit and push fix**

```bash
git add .github/workflows/ci.yml
git commit -m "fix(ci): <description of what was fixed>"
git push
```

- [ ] **Step 4: Verify CI passes**

Repeat until all 4 jobs are green.
