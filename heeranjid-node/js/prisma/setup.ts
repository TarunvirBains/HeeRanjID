import { readFileSync } from "fs";
import { join } from "path";

const SQL_DIR = join(__dirname, "..", "..", "sql");

function readSQL(filename: string): string {
  return readFileSync(join(SQL_DIR, filename), "utf-8");
}

/**
 * Returns the full install SQL (schema + functions) that must be executed
 * before HeeRanjID can generate IDs. Run this in a Prisma migration or
 * via `$executeRawUnsafe`.
 */
export function getInstallSQL(): string {
  return [
    readSQL("schema.sql"),
    readSQL("session.sql"),
    readSQL("generate_heerid.sql"),
    readSQL("generate_ranjid.sql"),
  ].join("\n");
}

/**
 * Returns the seed SQL that inserts a default node (node_id = 1).
 * Safe to run multiple times (uses ON CONFLICT DO NOTHING).
 */
export function getSeedSQL(): string {
  return readSQL("seed.sql");
}
