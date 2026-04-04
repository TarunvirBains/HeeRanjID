# SQL File Architecture Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate all duplicated/symlinked SQL files from binding packages so the `sql/` submodule is the single source of truth, with each package pulling SQL at build time.

**Architecture:** Remove per-package `sql/` directories (symlinks and `__init__.py` files). Configure each package's build tool to include SQL from the `sql/` submodule directly. Rust already does this correctly via `include_str!()`. Python uses maturin `include`, Node uses a `prepack` script, .NET uses MSBuild `Link`, and C/FFI uses a `build.rs` copy step.

**Tech Stack:** maturin (Python), npm/napi-rs (Node), MSBuild/.NET 8 (C#), cargo/cbindgen (Rust FFI)

**Spec:** `docs/superpowers/specs/2026-04-04-sql-architecture-design.md`

---

### Task 1: Python — Remove symlinks, configure maturin build-time inclusion

**Files:**
- Delete: `heeranjid-python/python/heeranjid/sql/` (entire directory tree)
- Modify: `heeranjid-python/pyproject.toml`

- [ ] **Step 1: Delete the symlinked sql directory**

```bash
rm -rf heeranjid-python/python/heeranjid/sql/
```

Verify it's gone:

```bash
ls heeranjid-python/python/heeranjid/sql/ 2>&1
# Expected: "No such file or directory"
```

- [ ] **Step 2: Update pyproject.toml to include SQL at build time**

In `heeranjid-python/pyproject.toml`, update the `[tool.maturin]` section:

```toml
[tool.maturin]
python-source = "python"
module-name = "heeranjid._heeranjid"
features = ["pyo3/extension-module"]
include = [
    { path = "../sql/postgres/**/*.sql", format = "wheel", to = "heeranjid/sql/postgres/" },
    { path = "../sql/mssql/**/*.sql", format = "wheel", to = "heeranjid/sql/mssql/" },
]
```

- [ ] **Step 3: Rebuild the package and verify SQL is included**

```bash
cd heeranjid-python
source ../.venv/bin/activate
maturin develop
```

Then verify the SQL files are accessible:

```bash
python -c "
from importlib import resources
sql_dir = resources.files('heeranjid.sql').joinpath('postgres')
print(sql_dir.joinpath('schema.sql').read_text(encoding='utf-8')[:80])
sql_dir = resources.files('heeranjid.sql').joinpath('mssql')
print(sql_dir.joinpath('schema.sql').read_text(encoding='utf-8')[:80])
print('OK: both backends accessible')
"
```

Expected: prints first 80 chars of each schema.sql and "OK: both backends accessible"

**Important:** If `importlib.resources` can't find the `heeranjid.sql` package (because the `__init__.py` files were deleted), we need to check whether maturin's `include` config places files in a way that creates a proper Python package. If not, we need `__init__.py` files generated at build time. See Step 4.

- [ ] **Step 4: Handle __init__.py for importlib.resources (if needed)**

If Step 3 fails with a package not found error, maturin may not auto-create `__init__.py` files. In that case, create them manually in the Python source and keep them in git — they contain no SQL, just mark the directories as Python packages:

Create `heeranjid-python/python/heeranjid/sql/__init__.py` (empty file):
```bash
mkdir -p heeranjid-python/python/heeranjid/sql/postgres
mkdir -p heeranjid-python/python/heeranjid/sql/mssql
touch heeranjid-python/python/heeranjid/sql/__init__.py
touch heeranjid-python/python/heeranjid/sql/postgres/__init__.py
touch heeranjid-python/python/heeranjid/sql/mssql/__init__.py
```

These directories will contain ONLY `__init__.py` — no SQL files. The SQL files are injected by maturin at build time.

Re-run Step 3 to verify.

- [ ] **Step 5: Run integration tests**

```bash
cd /home/tarunvir/projects/HeeRanjID
source .venv/bin/activate

# Postgres tests
DATABASE_URL='postgres://postgres:postgres@localhost:5432/heeranjid' \
  python -m pytest heeranjid-python/tests/test_postgres_integration.py -v

# MSSQL tests
MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;TrustServerCertificate=yes' \
  python -m pytest heeranjid-python/tests/test_mssql_integration.py -v

# Django field unit tests
python -m pytest heeranjid-python/tests/test_django_fields.py -v
```

Expected: all tests pass (6 postgres, 33 mssql, django field tests)

- [ ] **Step 6: Commit**

```bash
git add heeranjid-python/pyproject.toml
git add heeranjid-python/python/heeranjid/sql/  # only __init__.py files if they exist
git rm -r --cached heeranjid-python/python/heeranjid/sql/*.sql 2>/dev/null || true
git commit -m "refactor(python): remove SQL symlinks, use maturin build-time inclusion"
```

---

### Task 2: Node.js — Remove symlinks, update SQL path, add prepack script

**Files:**
- Delete: `heeranjid-node/sql/` (entire directory of symlinks)
- Modify: `heeranjid-node/js/prisma/setup.ts`
- Modify: `heeranjid-node/package.json`
- Modify: `heeranjid-node/.gitignore`

- [ ] **Step 1: Delete the symlinked sql directory**

```bash
rm -rf heeranjid-node/sql/
```

- [ ] **Step 2: Update setup.ts to read from the submodule**

Replace the contents of `heeranjid-node/js/prisma/setup.ts`:

```typescript
import { readFileSync } from "fs";
import { join } from "path";

// Resolve SQL directory: in development, read from the sql/ submodule;
// in an npm package, read from the bundled sql/ directory.
function getSqlDir(): string {
  const bundled = join(__dirname, "..", "..", "sql");
  const submodule = join(__dirname, "..", "..", "..", "sql");
  try {
    readFileSync(join(bundled, "postgres", "schema.sql"));
    return bundled;
  } catch {
    return submodule;
  }
}

const SQL_DIR = getSqlDir();

function readSQL(backend: string, filename: string): string {
  return readFileSync(join(SQL_DIR, backend, filename), "utf-8");
}

function readSQLSub(backend: string, subdir: string, filename: string): string {
  return readFileSync(join(SQL_DIR, backend, subdir, filename), "utf-8");
}

/**
 * Returns the full install SQL (schema + functions/procedures) for the
 * given backend. Run this in a Prisma migration or via `$executeRawUnsafe`.
 */
export function getInstallSQL(backend: string = "postgres"): string {
  const funcDir = backend === "mssql" ? "procedures" : "functions";
  return [
    readSQL(backend, "schema.sql"),
    readSQLSub(backend, funcDir, "session.sql"),
    readSQLSub(backend, funcDir, "generate_heerid.sql"),
    readSQLSub(backend, funcDir, "generate_ranjid.sql"),
  ].join("\n");
}

/**
 * Returns the seed SQL that inserts a default node (node_id = 1).
 * Safe to run multiple times.
 */
export function getSeedSQL(backend: string = "postgres"): string {
  return readSQL(backend, "seed.sql");
}
```

- [ ] **Step 3: Update package.json with prepack script and files list**

In `heeranjid-node/package.json`, add the `prepack` script and update `files`:

```json
{
  "name": "heeranjid",
  "version": "0.1.0",
  "description": "Distributed ID generation - HeerId (64-bit) and RanjId (128-bit UUIDv7)",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "license": "MIT",
  "napi": {
    "name": "heeranjid",
    "triples": {
      "defaults": true
    }
  },
  "scripts": {
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform",
    "prepack": "cp -r ../sql ./sql",
    "postpack": "rm -rf ./sql",
    "test": "vitest run"
  },
  "files": [
    "dist/",
    "js/",
    "sql/",
    "index.js",
    "index.d.ts"
  ],
  "devDependencies": {
    "@napi-rs/cli": "^2",
    "vitest": "^2",
    "typescript": "^5"
  },
  "peerDependencies": {
    "@prisma/client": ">=5.0.0"
  },
  "peerDependenciesMeta": {
    "@prisma/client": {
      "optional": true
    }
  }
}
```

- [ ] **Step 4: Add sql/ to .gitignore**

Append to `heeranjid-node/.gitignore`:

```
# SQL files are copied from submodule at pack time, not stored in repo
sql/
```

- [ ] **Step 5: Verify setup.ts resolves correctly in development**

```bash
cd /home/tarunvir/projects/HeeRanjID/heeranjid-node
npx ts-node -e "
const { getInstallSQL, getSeedSQL } = require('./js/prisma/setup');
console.log(getInstallSQL('postgres').substring(0, 80));
console.log(getSeedSQL('postgres').substring(0, 80));
console.log('OK');
"
```

Expected: prints first 80 chars of install and seed SQL and "OK"

If `ts-node` isn't available, verify manually:

```bash
node -e "
const fs = require('fs');
const path = require('path');
const sqlDir = path.join(__dirname, '..', 'sql');
console.log(fs.existsSync(path.join(sqlDir, 'postgres', 'schema.sql')));
console.log(fs.existsSync(path.join(sqlDir, 'mssql', 'schema.sql')));
"
```

Expected: `true`, `true`

- [ ] **Step 6: Commit**

```bash
git add heeranjid-node/js/prisma/setup.ts heeranjid-node/package.json heeranjid-node/.gitignore
git rm -r --cached heeranjid-node/sql/ 2>/dev/null || true
git commit -m "refactor(node): remove SQL symlinks, read from submodule directly"
```

---

### Task 3: .NET — Remove symlinks, use MSBuild Link

**Files:**
- Delete: `heeranjid-dotnet/src/HeeRanjID/Sql/` (entire directory of symlinks)
- Modify: `heeranjid-dotnet/src/HeeRanjID/HeeRanjID.csproj`
- Modify: `heeranjid-dotnet/src/HeeRanjID/SqlHelper.cs`

- [ ] **Step 1: Delete the symlinked Sql directory**

```bash
rm -rf heeranjid-dotnet/src/HeeRanjID/Sql/
```

- [ ] **Step 2: Update .csproj to embed from submodule via Link**

Replace the `<EmbeddedResource>` item group in `heeranjid-dotnet/src/HeeRanjID/HeeRanjID.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <RootNamespace>HeeRanjID</RootNamespace>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.EntityFrameworkCore" Version="8.0.*" />
  </ItemGroup>

  <ItemGroup>
    <EmbeddedResource Include="..\..\..\sql\**\*.sql"
                      Link="Sql\%(RecursiveDir)%(Filename)%(Extension)" />
  </ItemGroup>
</Project>
```

- [ ] **Step 3: Update SqlHelper.cs to support backend parameter**

Replace `heeranjid-dotnet/src/HeeRanjID/SqlHelper.cs`:

```csharp
using System.Reflection;

namespace HeeRanjID;

/// <summary>
/// Provides access to the embedded SQL migration scripts.
/// </summary>
public static class SqlHelper
{
    private static readonly Assembly ThisAssembly = typeof(SqlHelper).Assembly;

    /// <summary>
    /// Returns the full install SQL (schema + all functions/procedures)
    /// for the specified backend, concatenated into a single script.
    /// </summary>
    public static string GetInstallSql(string backend = "postgres")
        => string.Join("\n",
            GetSchemaSql(backend),
            GetSessionSql(backend),
            GetGenerateHeerIdSql(backend),
            GetGenerateRanjIdSql(backend));

    /// <summary>
    /// Returns the schema SQL for the specified backend.
    /// </summary>
    public static string GetSchemaSql(string backend = "postgres")
        => ReadResource($"HeeRanjID.Sql.{backend}.schema.sql");

    /// <summary>
    /// Returns the seed SQL for the specified backend.
    /// </summary>
    public static string GetSeedSql(string backend = "postgres")
        => ReadResource($"HeeRanjID.Sql.{backend}.seed.sql");

    /// <summary>
    /// Returns the generate_heerid function/procedure SQL.
    /// </summary>
    public static string GetGenerateHeerIdSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadResource($"HeeRanjID.Sql.{backend}.{subdir}.generate_heerid.sql");
    }

    /// <summary>
    /// Returns the generate_ranjid function/procedure SQL.
    /// </summary>
    public static string GetGenerateRanjIdSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadResource($"HeeRanjID.Sql.{backend}.{subdir}.generate_ranjid.sql");
    }

    /// <summary>
    /// Returns the session function/procedure SQL.
    /// </summary>
    public static string GetSessionSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadResource($"HeeRanjID.Sql.{backend}.{subdir}.session.sql");
    }

    /// <summary>
    /// Returns all available SQL resource names.
    /// </summary>
    public static string[] GetResourceNames()
        => ThisAssembly.GetManifestResourceNames()
            .Where(n => n.EndsWith(".sql", StringComparison.OrdinalIgnoreCase))
            .OrderBy(n => n)
            .ToArray();

    private static string ReadResource(string name)
    {
        using var stream = ThisAssembly.GetManifestResourceStream(name)
            ?? throw new InvalidOperationException($"Embedded resource '{name}' not found.");
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }
}
```

- [ ] **Step 4: Build and verify embedded resources**

```bash
cd /home/tarunvir/projects/HeeRanjID/heeranjid-dotnet
dotnet build src/HeeRanjID/HeeRanjID.csproj
```

Expected: build succeeds with no errors.

Then verify the resources are embedded correctly:

```bash
dotnet run --project src/HeeRanjID/ -- 2>/dev/null || \
  dotnet script -e "
    var names = HeeRanjID.SqlHelper.GetResourceNames();
    foreach (var n in names) Console.WriteLine(n);
  " 2>/dev/null || true
```

If the above doesn't work (no runnable project), verify by checking the build output for embedded resource warnings, or add a quick test:

```bash
cd /home/tarunvir/projects/HeeRanjID/heeranjid-dotnet
dotnet test 2>&1 | head -20
```

Expected: existing tests pass, confirming resources load correctly.

- [ ] **Step 5: Commit**

```bash
git add heeranjid-dotnet/src/HeeRanjID/HeeRanjID.csproj heeranjid-dotnet/src/HeeRanjID/SqlHelper.cs
git rm -r --cached heeranjid-dotnet/src/HeeRanjID/Sql/ 2>/dev/null || true
git commit -m "refactor(dotnet): remove SQL symlinks, embed from submodule via MSBuild Link"
```

---

### Task 4: C/FFI — Add build.rs step to copy SQL to output directory

**Files:**
- Modify: `heeranjid-ffi/build.rs`

- [ ] **Step 1: Update build.rs to copy SQL files to OUT_DIR**

Replace `heeranjid-ffi/build.rs`:

```rust
use std::path::Path;

fn main() {
    // Generate C header bindings
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file("heeranjid.h");

    // Copy SQL files from submodule to output directory
    let sql_src = Path::new(&crate_dir).join("../sql");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let sql_dest = Path::new(&out_dir).join("sql");

    if sql_src.exists() {
        copy_dir_recursive(&sql_src, &sql_dest)
            .expect("Failed to copy SQL files to output directory");
        println!("cargo:warning=SQL files copied to {}", sql_dest.display());
    }

    // Re-run build script if SQL files change
    println!("cargo:rerun-if-changed=../sql");
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            // Skip .git directories inside the submodule
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Build and verify SQL is in output**

```bash
cd /home/tarunvir/projects/HeeRanjID
cargo build -p heeranjid-ffi 2>&1 | grep "SQL files copied"
```

Expected: `warning: SQL files copied to /home/tarunvir/projects/HeeRanjID/target/debug/build/heeranjid-ffi-.../out/sql`

Verify the files exist:

```bash
find target/debug/build/heeranjid-ffi-*/out/sql -name "*.sql" | head -10
```

Expected: lists SQL files from both `postgres/` and `mssql/` subdirectories.

- [ ] **Step 3: Verify header generation still works**

```bash
ls -la heeranjid-ffi/heeranjid.h
```

Expected: header file exists and is recent.

- [ ] **Step 4: Commit**

```bash
git add heeranjid-ffi/build.rs
git commit -m "feat(ffi): copy SQL files from submodule to build output"
```

---

### Task 5: Verify all packages build and tests pass

**Files:** None (verification only)

- [ ] **Step 1: Run Rust workspace build**

```bash
cd /home/tarunvir/projects/HeeRanjID
cargo build --workspace
```

Expected: all crates build successfully.

- [ ] **Step 2: Run Rust tests**

```bash
cargo test -p heeranjid
```

Expected: all unit tests pass.

- [ ] **Step 3: Run Python tests**

```bash
source .venv/bin/activate

# Unit tests
python -m pytest heeranjid-python/tests/test_heerid.py heeranjid-python/tests/test_ranjid.py -v

# Django field tests
python -m pytest heeranjid-python/tests/test_django_fields.py -v

# Postgres integration
DATABASE_URL='postgres://postgres:postgres@localhost:5432/heeranjid' \
  python -m pytest heeranjid-python/tests/test_postgres_integration.py -v

# MSSQL integration
MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;TrustServerCertificate=yes' \
  python -m pytest heeranjid-python/tests/test_mssql_integration.py -v
```

Expected: all tests pass.

- [ ] **Step 4: Build .NET**

```bash
cd /home/tarunvir/projects/HeeRanjID/heeranjid-dotnet
dotnet build
```

Expected: build succeeds.

- [ ] **Step 5: Verify no SQL files remain in package directories (git status)**

```bash
cd /home/tarunvir/projects/HeeRanjID
# Should show NO .sql files inside any package directory
find heeranjid-python/python/heeranjid/sql -name "*.sql" 2>/dev/null
find heeranjid-node/sql -name "*.sql" 2>/dev/null
find heeranjid-dotnet/src/HeeRanjID/Sql -name "*.sql" 2>/dev/null
```

Expected: no output (no SQL files in package directories).

- [ ] **Step 6: Final commit if any cleanup needed**

```bash
git status
# If clean, skip this step
# If there are remaining tracked symlinks or files to remove:
git add -A
git commit -m "chore: clean up remaining SQL file references"
```
