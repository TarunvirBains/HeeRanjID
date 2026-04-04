# SQL File Architecture Redesign

## Goal

Eliminate all duplicated and symlinked SQL files from binding packages. The `sql/` git submodule is the single source of truth. Each package references it directly at build time to produce atomic, self-contained artifacts. No SQL files exist inside any package directory at rest in git.

## Problem

The `sql/` submodule contains both Postgres and MSSQL implementations. But each binding package (Python, Node, .NET) maintained its own copies or symlinks of these files inside its directory tree. This created:

- **Drift risk**: copies could fall out of sync with the submodule (which already caused a concurrency bug — the Python package had stale procedures without the `BEGIN TRANSACTION` fix).
- **Symlink fragility**: relative symlinks break if directory structure changes or if the submodule isn't checked out.
- **Unclear ownership**: SQL lived in both `sql/` and inside packages, making it ambiguous which was authoritative.
- **Future friction**: adding a new backend (e.g., MySQL) would require touching every package's directory structure.

## Design Principle

**At rest in git**: no SQL files exist inside any package directory. Only the `sql/` submodule holds SQL.

**At build time**: each package's build tool pulls SQL from `sql/` into the artifact (wheel, npm package, .NET assembly, Rust binary). The resulting artifact is atomic and self-contained — it includes the SQL it needs and nothing else.

**Future-proofing**: adding MSSQL support to Node or .NET means wiring up the build to include `sql/mssql/` and adding runtime backend detection. No restructuring needed.

## Per-Package Design

### Rust (heeranjid-sqlx) — No changes

Already correct. Uses `include_str!("../../sql/postgres/...")` to embed SQL at compile time directly from the submodule. No files inside the crate directory.

When sqlx gains MSSQL support, add a `mssql.rs` module with `include_str!("../../sql/mssql/...")`. Done.

### Python (heeranjid-python)

**Remove**: `python/heeranjid/sql/` directory entirely (symlinks, `__init__.py` files).

**Build-time inclusion**: Configure maturin to bundle SQL files from `../sql/` into the wheel. In `pyproject.toml`:

```toml
[tool.maturin]
data = [
    { src = "../sql/postgres/**/*.sql", dest = "heeranjid/sql/postgres/" },
    { src = "../sql/mssql/**/*.sql", dest = "heeranjid/sql/mssql/" },
]
```

If maturin's `data` config doesn't support this pattern, use the `include` directive or a custom build script that copies SQL into the wheel's package data at build time.

**Runtime loading**: Continue using `importlib.resources.files("heeranjid.sql.postgres")` and `importlib.resources.files("heeranjid.sql.mssql")` — these resolve to the bundled files inside the installed wheel. For editable/development installs (`maturin develop`), the build step still runs and places files correctly.

**Django migration**: No code changes needed to `0001_install_heeranjid.py` — `_read_sql()` still uses `importlib.resources`. The difference is where the files come from (build output vs. symlinked directory).

**Test fixture**: Update `test_mssql_integration.py` and `test_postgres_integration.py` fixtures if they reference `importlib.resources` for SQL loading — they should continue to work since the installed package will have the SQL.

### Node.js (heeranjid-node)

**Remove**: `heeranjid-node/sql/` directory entirely.

**Runtime (development)**: Update `setup.ts` to resolve SQL from `../sql/` relative to the package root:

```typescript
const SQL_DIR = join(__dirname, "..", "..", "..", "sql");

function readSQL(backend: string, filename: string): string {
  return readFileSync(join(SQL_DIR, backend, filename), "utf-8");
}

// For functions/procedures which live in subdirectories:
function readSQLFunction(backend: string, filename: string): string {
  const subdir = backend === "mssql" ? "procedures" : "functions";
  return readFileSync(join(SQL_DIR, backend, subdir, filename), "utf-8");
}
```

**npm packaging**: Add a `prepublish` or `prepack` script that copies `sql/` into the package directory before `npm pack`:

```json
{
  "scripts": {
    "prepack": "cp -r ../sql ./sql"
  },
  "files": ["js/", "sql/", "index.js", "index.d.ts"]
}
```

The copied `sql/` directory is included in the npm tarball but excluded from git (add `sql/` to `.gitignore` inside `heeranjid-node/`).

### .NET (heeranjid-dotnet)

**Remove**: `src/HeeRanjID/Sql/` directory entirely.

**Build-time embedding**: Update `.csproj` to embed SQL directly from the submodule using MSBuild `Link`:

```xml
<ItemGroup>
  <EmbeddedResource Include="..\..\..\sql\**\*.sql"
                    Link="Sql\%(RecursiveDir)%(Filename)%(Extension)" />
</ItemGroup>
```

This embeds all SQL files (both backends) into the assembly at compile time. The `Link` attribute controls the logical path, so resource names become:

- `HeeRanjID.Sql.postgres.schema.sql`
- `HeeRanjID.Sql.postgres.functions.generate_heerid.sql`
- `HeeRanjID.Sql.mssql.schema.sql`
- `HeeRanjID.Sql.mssql.procedures.generate_heerid.sql`

**SqlHelper.cs**: Update resource name constants to match the new link structure. Add a `backend` parameter to methods like `GetInstallSql(string backend = "postgres")` so MSSQL can be used when ready.

## Build Dependencies

Each package build requires the SQL submodule plus its Rust dependency chain:

| Package | Rust dependency | SQL | Build output |
|---------|----------------|-----|--------------|
| heeranjid-ffi | `heeranjid` (Cargo dependency) | `sql/` copied as raw SQL alongside binary | `.so`/`.dylib`/`.dll` + `.h` + `sql/` |
| heeranjid-python | `heeranjid` (via PyO3) | `sql/` bundled into wheel | `.whl` |
| heeranjid-node | `heeranjid` (via NAPI-RS) | `sql/` copied at prepack | npm tarball |
| heeranjid-dotnet | `heeranjid-ffi` (via P/Invoke, C ABI) | `sql/` embedded via Link | NuGet `.nupkg` |
| heeranjid-sqlx | `heeranjid` (Cargo dependency) | `sql/` via `include_str!` | Rust crate |

Each build is atomic — it only needs its own source, the Rust crate(s) it depends on, and `sql/`. The .NET package doesn't need the Python source. The Node package doesn't need the sqlx crate.

## Directory Structure After

```
HeeRanjID/
├── sql/                          # git submodule — single source of truth
│   ├── postgres/
│   │   ├── schema.sql
│   │   ├── seed.sql
│   │   ├── install.sql
│   │   ├── functions/
│   │   │   ├── session.sql
│   │   │   ├── generate_heerid.sql
│   │   │   └── generate_ranjid.sql
│   │   └── queries/
│   │       ├── fetch_node.sql
│   │       ├── fetch_epoch.sql
│   │       └── fetch_active_node.sql
│   └── mssql/
│       ├── schema.sql
│       ├── seed.sql
│       ├── install.sql
│       ├── procedures/
│       │   ├── session.sql
│       │   ├── generate_heerid.sql
│       │   └── generate_ranjid.sql
│       └── queries/
│           ├── fetch_node.sql
│           ├── fetch_epoch.sql
│           └── fetch_active_node.sql
├── heeranjid/                    # Rust core — no SQL
├── heeranjid-ffi/                # C FFI — no SQL (types only)
├── heeranjid-sqlx/               # Rust sqlx — include_str! from sql/
│   └── src/postgres.rs
├── heeranjid-python/             # Python — NO sql/ directory
│   ├── python/heeranjid/
│   │   ├── django/
│   │   └── (no sql/ directory)
│   └── pyproject.toml            # maturin bundles sql/ at build time
├── heeranjid-node/               # Node — NO sql/ directory
│   ├── js/
│   └── package.json              # prepack copies sql/ for npm tarball
└── heeranjid-dotnet/             # .NET — NO Sql/ directory
    └── src/HeeRanjID/
        ├── HeeRanjID.csproj      # MSBuild Link embeds from sql/
        └── SqlHelper.cs
```

## What Changes Per Package

| Package | Remove | Add/Update |
|---------|--------|------------|
| heeranjid-sqlx | nothing | nothing (already correct) |
| heeranjid-python | `python/heeranjid/sql/` (symlinks + `__init__.py`) | `pyproject.toml` build-time inclusion config |
| heeranjid-node | `heeranjid-node/sql/` (symlinks) | `package.json` prepack script, update `setup.ts` path |
| heeranjid-dotnet | `src/HeeRanjID/Sql/` (symlinks) | `.csproj` Link directive, update `SqlHelper.cs` |

## Testing

- All existing integration tests continue to pass (they load SQL through the package's runtime mechanism, which is backed by the build output).
- The Postgres integration tests (Python 6 tests, Rust concurrency tests) verify Postgres SQL is correctly included.
- The MSSQL integration tests (Python 33 tests) verify MSSQL SQL is correctly included.
- No new tests needed — this is a build/packaging change, not a runtime behavior change.

## Future: Adding a New Backend

To add MSSQL support to Node or .NET:

1. The SQL already exists in `sql/mssql/` — no submodule changes needed.
2. Add backend detection to the package's runtime code (like Python's `connection.vendor` check).
3. Update the SQL loading to accept a backend parameter.
4. Add integration tests against the MSSQL Docker container.

No restructuring, no new directories, no new symlinks. Just wiring.
