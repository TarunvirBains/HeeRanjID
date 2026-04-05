import { existsSync, readFileSync } from "fs";
import { join } from "path";

type Backend = "postgres" | "mssql";

/**
 * Resolves the SQL directory root.
 * - When installed as an npm package, sql/ is bundled alongside this file (copied at pack time).
 * - In development (repo checkout), sql/ lives at the repo root as the git submodule.
 */
function resolveSqlRoot(): string {
  // __dirname is bindings/node/js/prisma in both dev and built scenarios.
  // Bundled path: bindings/node/sql/ (two dirs up from __dirname)
  const bundled = join(__dirname, "..", "..", "sql");
  if (existsSync(bundled)) {
    return bundled;
  }
  // Development / submodule path: repo root sql/ (four dirs up from __dirname)
  const submodule = join(__dirname, "..", "..", "..", "..", "sql");
  if (existsSync(submodule)) {
    return submodule;
  }
  throw new Error(
    `HeeRanjID: cannot locate SQL directory. Expected bundled at "${bundled}" or submodule at "${submodule}".`
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
 * Returns the full install SQL (schema + stored routines) that must be
 * executed before HeeRanjID can generate IDs.  Run this in a Prisma
 * migration or via `$executeRawUnsafe`.
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
  ].join("\n");
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
