import { existsSync, readFileSync } from "fs";
import { join } from "path";

type Backend = "postgres" | "mssql";

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

  // __dirname is bindings/typescript/prisma/src in dev and bindings/typescript/prisma/dist in built scenarios.
  // Bundled path: bindings/typescript/prisma/sql/ (two dirs up from __dirname)
  const bundled = join(__dirname, "..", "..", "sql");
  if (existsSync(bundled)) {
    return bundled;
  }
  // Development / submodule path: repo root sql/ (four dirs up from __dirname,
  // e.g. bindings/typescript/prisma/src -> ../../../../sql)
  const submodule4 = join(__dirname, "..", "..", "..", "..", "sql");
  if (existsSync(submodule4)) {
    return submodule4;
  }
  // Alternative submodule path (five dirs up, for nested checkout layouts)
  const submodule5 = join(__dirname, "..", "..", "..", "..", "..", "sql");
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
 * @param backend - Database backend. Defaults to "postgres".
 */
export function getInstallSQL(backend: Backend = "postgres"): string {
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
 * @param backend - Database backend. Defaults to "postgres".
 */
export function getConfigureSQL(backend: Backend = "postgres"): string {
  const sqlRoot = resolveSqlRoot();
  return readRoutineSQL(sqlRoot, backend, "configure.sql");
}

/**
 * Returns the seed SQL that inserts a default node (node_id = 1).
 * Safe to run multiple times (uses ON CONFLICT DO NOTHING for postgres,
 * MERGE for mssql).
 *
 * @param backend - Database backend. Defaults to "postgres".
 */
export function getSeedSQL(backend: Backend = "postgres"): string {
  const sqlRoot = resolveSqlRoot();
  return readSQL(sqlRoot, backend, "seed.sql");
}
