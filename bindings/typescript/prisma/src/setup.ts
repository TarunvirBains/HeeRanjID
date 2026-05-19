import { existsSync, readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { type Backend, assertBackend } from "./validators.js";

/**
 * Directory containing this source / built file.
 *
 * Derived from `import.meta.url` because `heeranjid-prisma` is an
 * **ESM-only** package (`"type": "module"` with only an ESM `main:`).
 * The bare `__dirname` global is undefined under ESM — using it would
 * crash with `ReferenceError: __dirname is not defined` at the first
 * `install()` call. `import.meta.url` is the ESM-native equivalent and
 * resolves to the on-disk URL of the running module under Node 20+'s
 * Node16 module resolution.
 *
 * CJS consumers cannot `require()` this package directly (Node would
 * raise `ERR_REQUIRE_ESM`). They must use dynamic `import()`:
 *
 * ```ts
 * const { heeranjidExtension } = await import("heeranjid-prisma");
 * ```
 *
 * The base `heeranjid` package supports both module systems via
 * napi-rs's dual interop, so consumer code that does not depend on the
 * Prisma extension can keep using `require()`.
 */
const moduleDir = dirname(fileURLToPath(import.meta.url));

/**
 * Resolves the SQL directory root.
 * - When installed as an npm package, sql/ is bundled alongside this file (copied at pack time).
 * - In development (repo checkout), sql/ lives at the repo root as the git submodule.
 * - The HEERANJID_SQL_DIR environment variable can override the resolved path.
 */
function resolveSqlRoot(): string {
  // Allow explicit override via environment variable (useful in tests and CI).
  const envOverride = process.env.HEERANJID_SQL_DIR;
  if (envOverride && existsSync(envOverride)) {
    return envOverride;
  }

  // `moduleDir` is:
  //   - dev:      bindings/typescript/prisma/src (when imported via the
  //               vitest .ts resolver plugin)
  //   - built:    bindings/typescript/prisma/dist (after `tsc` with
  //               `rootDir: "src"`, `outDir: "dist"`, so source files emit
  //               to `dist/index.js` / `dist/setup.js`, NOT `dist/src/…`)
  //   - packed:   <consumer>/node_modules/heeranjid-prisma/dist
  //
  // Bundled path: `prisma/sql/` lives ONE dir above `moduleDir` in both
  // the built dev tree and the unpacked tarball — `prepack` populates
  // `prisma/sql/` before `npm pack` zips the package, and the published
  // layout therefore matches the built layout. Two dirs up would land
  // OUTSIDE the package boundary (in the consumer's `node_modules/`).
  const bundled = join(moduleDir, "..", "sql");
  if (existsSync(bundled)) {
    return bundled;
  }
  // Development / submodule path: repo root sql/. Both `prisma/src/`
  // and `prisma/dist/` are 4 dirs below the repo root (… /bindings/
  // typescript/prisma/{src,dist}), so the same probe handles both
  // dev-time vitest (which imports the `.ts` source) and any built-tree
  // experiment (`node dist/setup.js`).
  const submodule4 = join(moduleDir, "..", "..", "..", "..", "sql");
  if (existsSync(submodule4)) {
    return submodule4;
  }
  // Alternative submodule path (five dirs up, for nested checkout layouts)
  const submodule5 = join(moduleDir, "..", "..", "..", "..", "..", "sql");
  if (existsSync(submodule5)) {
    return submodule5;
  }
  throw new Error(
    `HeeRanjID: cannot locate SQL directory. Expected bundled at "${bundled}" or submodule at "${submodule4}" or "${submodule5}". ` +
      `Set HEERANJID_SQL_DIR to override.`
  );
}

function readSQL(sqlRoot: string, backend: Backend, filename: string): string {
  return readFileSync(join(sqlRoot, backend, filename), "utf-8");
}

function readRoutineSQL(
  sqlRoot: string,
  backend: Backend,
  filename: string
): string {
  const routineDir = backend === "mssql" ? "procedures" : "functions";
  return readFileSync(join(sqlRoot, backend, routineDir, filename), "utf-8");
}

/**
 * Returns the full install SQL (schema + stored routines + configure function)
 * that must be executed before HeeRanjID can generate IDs.  Run this in a
 * Prisma migration or via `$executeRawUnsafe`.
 *
 * @param backend - Database backend. **Required** — must be either
 *   `"postgres"` or `"mssql"`. Mirrors the `withAutoIds` policy: a silent
 *   `"postgres"` default would cause an MSSQL consumer that forgot to pass
 *   `"mssql"` to install a Postgres-shaped schema against a sqlserver
 *   datasource, and the failure would surface only at the first
 *   `$executeRawUnsafe` call with a dialect-mismatch error that does not
 *   point at the misconfiguration. Forcing the caller to spell the
 *   backend out makes the mismatch a TypeScript compile error instead.
 */
export function getInstallSQL(backend: Backend): string {
  // Runtime validation (V3): JS callers / `any` casts can pass invalid
  // strings that would silently route to the postgres branch in
  // `readSQL` / `readRoutineSQL` (which hardcodes `"functions"` for
  // anything not `"mssql"`). Validating up front throws a clear error
  // instead of producing a file-not-found at a downstream readFileSync.
  assertBackend(backend, "getInstallSQL");
  const sqlRoot = resolveSqlRoot();
  return [
    readSQL(sqlRoot, backend, "schema.sql"),
    readRoutineSQL(sqlRoot, backend, "session.sql"),
    readRoutineSQL(sqlRoot, backend, "generate_heerid.sql"),
    readRoutineSQL(sqlRoot, backend, "generate_ranjid.sql"),
    readRoutineSQL(sqlRoot, backend, "configure.sql"),
  ].join("\n");
}

/**
 * Returns just the configure function/procedure SQL.
 * This is the SQL that defines `heer_configure()` (Postgres) or
 * `heer_configure` (MSSQL).
 *
 * @param backend - Database backend. **Required** — must be either
 *   `"postgres"` or `"mssql"`. See {@link getInstallSQL} for the
 *   rationale (consistent with the `withAutoIds` policy of forbidding a
 *   silent `"postgres"` default that would silently mismatch sqlserver
 *   consumers).
 */
export function getConfigureSQL(backend: Backend): string {
  // Runtime validation (V3): see `getInstallSQL` for the rationale.
  assertBackend(backend, "getConfigureSQL");
  const sqlRoot = resolveSqlRoot();
  return readRoutineSQL(sqlRoot, backend, "configure.sql");
}

/**
 * Returns the seed SQL that inserts a default node (node_id = 1).
 * Safe to run multiple times (uses ON CONFLICT DO NOTHING for postgres,
 * MERGE for mssql).
 *
 * @param backend - Database backend. **Required** — must be either
 *   `"postgres"` or `"mssql"`. See {@link getInstallSQL} for the
 *   rationale (consistent with the `withAutoIds` policy of forbidding a
 *   silent `"postgres"` default that would silently mismatch sqlserver
 *   consumers).
 */
export function getSeedSQL(backend: Backend): string {
  // Runtime validation (V3): see `getInstallSQL` for the rationale.
  assertBackend(backend, "getSeedSQL");
  const sqlRoot = resolveSqlRoot();
  return readSQL(sqlRoot, backend, "seed.sql");
}
