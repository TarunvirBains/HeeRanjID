/**
 * Database backend discriminator. Selects per-backend SQL dialect for
 * `$heeranjid` methods (e.g. `generate_ranjids` calling convention and
 * the wire shape of returned id columns).
 *
 * Note: this extension uses the labels `"postgres"` and `"mssql"`, which
 * intentionally diverge from Prisma's datasource `provider` names
 * (`"postgresql"` and `"sqlserver"`). The shorter labels match the
 * convention used throughout the HeeRanjID workspace — the Rust crate's
 * `mssql_schema` / `postgres_schema` modules, the SQL submodule layout
 * (`sql/postgres/`, `sql/mssql/`), and the .NET sibling binding's
 * internal backend discriminator. Consumers must translate from the
 * Prisma provider name themselves when configuring the extension.
 *
 * Defined in `validators.ts` (the shared validator module) so that both
 * `index.ts` and `setup.ts` can reference the canonical definition and
 * its runtime guard without a dependency cycle. Re-exported from
 * `index.ts` as a public API type.
 */
export type Backend = "postgres" | "mssql";

/**
 * ID kind discriminator for `withAutoIds` model maps.
 *
 * Either `"heerid"` (for HeeId / bigint columns) or `"ranjid"` (for
 * RanjId / uuid-or-binary columns). Defined here alongside
 * {@link assertIdKind} so both guards share one canonical location.
 */
export type IdKind = "heerid" | "ranjid";

/**
 * Asserts that an arbitrary value is a valid {@link Backend} discriminator.
 *
 * TypeScript narrows `Backend` to `"postgres" | "mssql"` at compile time,
 * but JS callers, `any` casts, or callers passing Prisma's own provider
 * name (`"sqlserver"`) silently fall through to the postgres branch
 * without this runtime check. The mismatch surfaces only at the first
 * dialect-specific `$executeRawUnsafe` call with a confusing
 * type/syntax error that does not point at the misconfiguration.
 *
 * Throws a `TypeError` with a message that includes the offending value
 * and a hint about Prisma's `"sqlserver"` naming so wrong-but-plausible
 * inputs are diagnosed instead of routed to the default branch.
 *
 * @param value   The value to validate.
 * @param context Caller-supplied prefix used in the error message (e.g.
 *   `"heeranjidExtension"`, `"install"`, `"getInstallSQL"`) so the
 *   thrown error points at the call site.
 *
 * @throws {TypeError} When `value` is not `"postgres"` or `"mssql"`.
 */
export function assertBackend(value: unknown, context: string): asserts value is Backend {
  if (value !== "postgres" && value !== "mssql") {
    throw new TypeError(
      `${context}: backend must be "postgres" or "mssql", got ${JSON.stringify(value)}. ` +
        `Note: Prisma's MSSQL provider is named "sqlserver" but heeranjid-prisma uses "mssql"; ` +
        `see README "Backend label convention" table.`
    );
  }
}

/**
 * Asserts that an arbitrary value is a valid {@link IdKind} discriminator
 * (i.e. either `"heerid"` or `"ranjid"`).
 *
 * Mirrors the rationale of {@link assertBackend}: TypeScript narrows the
 * map values at compile time, but JS callers can pass anything and a
 * silent fall-through to one branch would inject the wrong wire shape
 * into the column.
 *
 * @param value     The value to validate.
 * @param modelName The Prisma model name the map entry was looked up
 *   for, included in the error message so the failure points at the
 *   misconfigured key (e.g. `"withAutoIds models.User"`).
 *
 * @throws {TypeError} When `value` is not `"heerid"` or `"ranjid"`.
 */
export function assertIdKind(value: unknown, modelName: string): asserts value is IdKind {
  if (value !== "heerid" && value !== "ranjid") {
    throw new TypeError(
      `withAutoIds models.${modelName}: idKind must be "heerid" or "ranjid", got ${JSON.stringify(value)}.`
    );
  }
}
