import { HeerId, RanjId } from "heeranjid";
import { getInstallSQL, getSeedSQL, getConfigureSQL } from "./setup.js";
import { type Backend, assertBackend, assertIdKind } from "./validators.js";
export type { Backend } from "./validators.js";

/**
 * Shape expected from Prisma's `$queryRaw` for a single HeerId row.
 */
interface HeerIdRow {
  id: bigint;
}

/**
 * Postgres `RanjId` row shape: `id::text` cast yields a UUID string.
 */
interface PostgresRanjIdRow {
  id: string;
}

/**
 * MSSQL `RanjId` row shape: the `id` column is `BINARY(16)` and surfaces
 * through Prisma 6+'s sqlserver driver as a bare `Uint8Array`. (Prisma 5
 * used `Buffer`, which is a Uint8Array subclass — declaring `Uint8Array`
 * therefore accepts both shapes.) Routing through `Guid` would apply the
 * mixed-endian swizzle and corrupt the RanjId byte sequence; use
 * `RanjId.fromBytes` directly to preserve the canonical big-endian wire
 * format.
 */
interface MssqlRanjIdRow {
  id: Uint8Array;
}

/**
 * Reads the NODE_ID environment variable and returns it as a number.
 * Throws if the variable is not set.
 */
function getNodeId(): number {
  const nodeId = process.env.NODE_ID;
  if (!nodeId) {
    throw new Error("NODE_ID environment variable must be set");
  }
  return parseInt(nodeId, 10);
}

/**
 * Splits an MSSQL script on `GO` batch separators.
 *
 * `GO` is a sqlcmd/SSMS-only batch separator and is **not** a T-SQL
 * statement: the ODBC/OLE DB driver Prisma uses for sqlserver does not
 * understand it. Scripts that mix DDL with `CREATE OR ALTER PROCEDURE`
 * blocks (as the HeeRanjID install/seed SQL does) therefore must be
 * issued as one `$executeRawUnsafe` call **per batch**.
 *
 * The split pattern matches a line containing only `GO` (optionally with
 * trailing whitespace), with `m` so `^`/`$` anchor to line boundaries.
 * Empty batches (between adjacent `GO`s, or at the file edges) are
 * filtered out — passing an empty string to `$executeRawUnsafe` is a
 * driver error on most providers.
 *
 * @internal Exported for test introspection only; consumers should use
 *   the `install()` method on the `$heeranjid` extension, which calls
 *   this internally on the MSSQL path.
 */
export function splitMssqlBatches(sql: string): string[] {
  return sql
    .split(/^GO\s*$/m)
    .map((batch) => batch.trim())
    .filter((batch) => batch.length > 0);
}

/**
 * Options accepted by {@link heeranjidExtension}.
 */
export interface HeeranjidExtensionOptions {
  /**
   * Database backend dialect used by `$heeranjid` methods. Defaults to
   * `"postgres"`. Use `"mssql"` when extending a PrismaClient configured
   * with the `sqlserver` provider — this switches the SQL shape from
   * Postgres `SELECT func($1)` / `id::text` to MSSQL
   * `EXEC proc @P1, @P2` / `BINARY(16)` Uint8Array rows (Prisma 6+).
   */
  backend?: Backend;
}

/**
 * The extension methods added by `heeranjidExtension()`.
 */
export interface HeeranjidClient {
  $heeranjid: {
    /**
     * Set the HeerId node for this session. Must be called before
     * generating IDs (unless you pass node_id explicitly to generate).
     */
    setNodeId(nodeId: number): Promise<void>;

    /**
     * Set the RanjId node for this session.
     */
    setRanjNodeId(nodeId: number): Promise<void>;

    /**
     * Generate a single HeerId. Uses nodeId if provided, otherwise falls
     * back to the NODE_ID environment variable.
     */
    generateHeerId(nodeId?: number): Promise<HeerId>;

    /**
     * Generate multiple HeerIds.
     */
    generateHeerIds(nodeId: number, count: number): Promise<HeerId[]>;

    /**
     * Generate a single RanjId. Uses nodeId if provided, otherwise falls
     * back to the NODE_ID environment variable.
     */
    generateRanjId(nodeId?: number): Promise<RanjId>;

    /**
     * Generate multiple RanjIds.
     */
    generateRanjIds(nodeId: number, count: number): Promise<RanjId[]>;

    /**
     * Install the HeeRanjID schema, functions, seed data, and call
     * heer_configure(). Safe to call in a migration.
     *
     * @param backend - Database backend. If omitted, falls back to the
     *   backend supplied to {@link heeranjidExtension}; if that was also
     *   omitted, defaults to `"postgres"`.
     */
    install(backend?: Backend): Promise<void>;
  };
}

/**
 * Creates a Prisma client extension that adds `$heeranjid` methods for
 * distributed ID generation.
 *
 * The `backend` option (default `"postgres"`) selects the SQL dialect used
 * by every `$heeranjid` method. For MSSQL the extension issues `EXEC`
 * statements against the `heer_set_node_id` / `generate_ranjids` procedures
 * and decodes `BINARY(16)` rows via {@link RanjId.fromBytes}, preserving the
 * canonical big-endian wire format. For Postgres the extension keeps the
 * legacy `SELECT func($1)` calling convention with `id::text` casts.
 *
 * Usage:
 * ```ts
 * import { PrismaClient } from "@prisma/client";
 * import { heeranjidExtension } from "heeranjid-prisma";
 *
 * // Postgres (default)
 * const prisma = new PrismaClient().$extends(heeranjidExtension());
 *
 * // MSSQL
 * const prisma = new PrismaClient().$extends(
 *   heeranjidExtension({ backend: "mssql" })
 * );
 *
 * const id = await prisma.$heeranjid.generateHeerId(1);
 * ```
 */
export function heeranjidExtension(
  options: HeeranjidExtensionOptions = {}
) {
  // Runtime validation (V1): TS narrows `options.backend` to `Backend |
  // undefined`, but JS callers / `any` casts can pass arbitrary strings
  // (`"sqlserver"`, `"mysql"`) that would silently fall through to the
  // postgres branch and emit wrong SQL against the consumer's database.
  // We only validate when the caller explicitly supplied a value —
  // omission is fine and uses the documented `"postgres"` default.
  if (options.backend !== undefined) {
    assertBackend(options.backend, "heeranjidExtension");
  }
  const extensionBackend: Backend = options.backend ?? "postgres";
  // We use a dynamic import-style approach so this works without
  // @prisma/client being a hard dependency.
  return {
    name: "heeranjid",
    client: {
      $heeranjid: {
        // `this` is bound to the extended PrismaClient at runtime by Prisma.
        // We use `any` to avoid requiring @prisma/client types at compile time.

        async setNodeId(nodeId: number): Promise<void> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          if (extensionBackend === "mssql") {
            // sqlserver positional params use @P1 substitution; Prisma binds
            // $queryRawUnsafe / $executeRawUnsafe params 1-indexed in order.
            await client.$executeRawUnsafe(
              `EXEC heer_set_node_id @P1`,
              nodeId
            );
          } else {
            await client.$executeRawUnsafe(
              `SELECT set_heer_node_id($1)`,
              nodeId
            );
          }
        },

        async setRanjNodeId(nodeId: number): Promise<void> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          if (extensionBackend === "mssql") {
            await client.$executeRawUnsafe(
              `EXEC heer_set_ranj_node_id @P1`,
              nodeId
            );
          } else {
            await client.$executeRawUnsafe(
              `SELECT set_heer_ranj_node_id($1)`,
              nodeId
            );
          }
        },

        async generateHeerId(nodeId?: number): Promise<HeerId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const resolvedNodeId = nodeId ?? getNodeId();
          let rows: HeerIdRow[];
          if (extensionBackend === "mssql") {
            // MSSQL stored procedure returns a single result set via the
            // procedure's final SELECT. Prisma surfaces it through
            // $queryRawUnsafe just like a regular SELECT.
            rows = await client.$queryRawUnsafe(
              `EXEC generate_ids @in_node_id = @P1, @requested_count = 1`,
              resolvedNodeId
            );
          } else {
            rows = await client.$queryRawUnsafe(
              `SELECT id FROM generate_ids($1, 1)`,
              resolvedNodeId
            );
          }
          if (rows.length === 0) {
            throw new Error("generate_ids returned no rows");
          }
          return HeerId.fromBigInt(rows[0].id);
        },

        async generateHeerIds(
          nodeId: number,
          count: number
        ): Promise<HeerId[]> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          let rows: HeerIdRow[];
          if (extensionBackend === "mssql") {
            rows = await client.$queryRawUnsafe(
              `EXEC generate_ids @in_node_id = @P1, @requested_count = @P2`,
              nodeId,
              count
            );
          } else {
            rows = await client.$queryRawUnsafe(
              `SELECT id FROM generate_ids($1, $2)`,
              nodeId,
              count
            );
          }
          return rows.map((row) => HeerId.fromBigInt(row.id));
        },

        async generateRanjId(nodeId?: number): Promise<RanjId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const resolvedNodeId = nodeId ?? getNodeId();
          if (extensionBackend === "mssql") {
            // MSSQL stores RanjIds as BINARY(16) (big-endian); the procedure
            // returns the raw bytes which Prisma 6+'s sqlserver adapter
            // surfaces as a bare `Uint8Array`. Decode via fromBytes to skip
            // the Guid mixed-endian swizzle that would corrupt the sort key.
            const rows: MssqlRanjIdRow[] = await client.$queryRawUnsafe(
              `EXEC generate_ranjids @in_node_id = @P1, @requested_count = 1`,
              resolvedNodeId
            );
            if (rows.length === 0) {
              throw new Error("generate_ranjids returned no rows");
            }
            return RanjId.fromBytes(rows[0].id);
          }
          const rows: PostgresRanjIdRow[] = await client.$queryRawUnsafe(
            `SELECT id::text FROM generate_ranjids($1, 1)`,
            resolvedNodeId
          );
          if (rows.length === 0) {
            throw new Error("generate_ranjids returned no rows");
          }
          return RanjId.fromString(rows[0].id);
        },

        async generateRanjIds(
          nodeId: number,
          count: number
        ): Promise<RanjId[]> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          if (extensionBackend === "mssql") {
            const rows: MssqlRanjIdRow[] = await client.$queryRawUnsafe(
              `EXEC generate_ranjids @in_node_id = @P1, @requested_count = @P2`,
              nodeId,
              count
            );
            return rows.map((row) => RanjId.fromBytes(row.id));
          }
          const rows: PostgresRanjIdRow[] = await client.$queryRawUnsafe(
            `SELECT id::text FROM generate_ranjids($1, $2)`,
            nodeId,
            count
          );
          return rows.map((row) => RanjId.fromString(row.id));
        },

        async install(backend?: Backend): Promise<void> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          // Runtime validation (V2): validate the explicit override when
          // present; `extensionBackend` was already validated at
          // construction time so the fall-through path is safe.
          if (backend !== undefined) {
            assertBackend(backend, "install");
          }
          // install() accepts an explicit backend override for backward
          // compatibility; if omitted, fall back to the extension-level
          // backend captured at heeranjidExtension() construction.
          const resolvedBackend: Backend = backend ?? extensionBackend;

          const installSQL = getInstallSQL(resolvedBackend);
          const seedSQL = getSeedSQL(resolvedBackend);

          if (resolvedBackend === "mssql") {
            // MSSQL: split on `GO` batch separators. Prisma's
            // `$executeRawUnsafe` (which goes through the sqlserver ODBC
            // driver) accepts only single statements; `GO` is a sqlcmd /
            // SSMS batch separator and is not understood by ODBC/OLE DB.
            // Each batch (schema CREATE, procedure CREATE/ALTER, etc.) must
            // be issued as its own `$executeRawUnsafe` call.
            for (const batch of splitMssqlBatches(installSQL)) {
              await client.$executeRawUnsafe(batch);
            }
            for (const batch of splitMssqlBatches(seedSQL)) {
              await client.$executeRawUnsafe(batch);
            }
            await client.$executeRawUnsafe(`EXEC heer_configure`);
          } else {
            // Postgres: the install/seed SQL is a single statement-list
            // batch with no `GO` separators; issue it as one call.
            await client.$executeRawUnsafe(installSQL);
            await client.$executeRawUnsafe(seedSQL);
            await client.$executeRawUnsafe(`SELECT heer_configure()`);
          }
        },
      },
    },
  };
}

export { getNodeId, getInstallSQL, getSeedSQL, getConfigureSQL };

// ---------------------------------------------------------------------------
// withAutoIds — composable Prisma extension for automatic ID injection
// ---------------------------------------------------------------------------

/** Map of Prisma model names to the ID type they use. */
export type AutoIdModelMap = Record<string, "heerid" | "ranjid">;

export interface AutoIdConfig {
  /**
   * The node ID to use when generating IDs from the database.
   * Defaults to the NODE_ID environment variable if not provided.
   */
  nodeId?: number;
  /**
   * Map of model names (as they appear in `prisma.modelName`) to the ID type.
   * Models not listed here are not affected.
   *
   * @example
   * { User: "heerid", Post: "ranjid" }
   */
  models: AutoIdModelMap;
  /**
   * The name of the primary key field. Defaults to `"id"`.
   */
  idField?: string;
  /**
   * Database backend dialect used to select the wire shape that Prisma
   * will accept for the generated id values. **Required** — must match the
   * backend you passed to `heeranjidExtension()`.
   *
   * - `"postgres"`:
   *   - `HeerId` columns are written as `bigint` via `HeerId.toBigInt()`.
   *   - `RanjId` columns are written as `uuid` via `RanjId.toString()`
   *     (canonical hyphenated lowercase form).
   * - `"mssql"`:
   *   - `HeerId` columns are written as `bigint` via `HeerId.toBigInt()`.
   *   - `RanjId` columns are written as `BINARY(16)` via
   *     `RanjId.toBytes()` (canonical big-endian `Uint8Array`).
   *
   * Why required (no default): if `withAutoIds` silently defaulted to
   * `"postgres"` while paired with `heeranjidExtension({ backend: "mssql" })`,
   * RanjIds would be injected as UUID strings into a `BINARY(16)` column and
   * the sqlserver driver would reject the insert with a type error that
   * does not point at the misconfiguration. Forcing the caller to spell the
   * backend out makes the mismatch a TypeScript compile error instead of a
   * runtime surprise.
   */
  backend: Backend;
}

/**
 * Returns a Prisma Client Extension that automatically generates HeeRanjID
 * values for `create` and `createMany` operations when the primary key field
 * is absent or `undefined`.
 *
 * Must be composed **after** `heeranjidExtension()` so that `this.$heeranjid`
 * is available in the query interceptors. The `backend` option on both
 * extensions must match — see {@link AutoIdConfig.backend} for the rationale.
 *
 * The generated `HeerId` / `RanjId` class instances are serialized to the
 * wire shape Prisma's query engine accepts for the configured backend
 * **before** being injected into `args.data`:
 *
 * - `HeerId` → `bigint` (via `HeerId.toBigInt()`) on both Postgres and
 *   MSSQL — both back the column with `bigint`.
 * - `RanjId` → `string` (canonical UUID, via `RanjId.toString()`) on
 *   Postgres `uuid` columns; `Uint8Array` (16 big-endian bytes, via
 *   `RanjId.toBytes()`) on MSSQL `BINARY(16)` columns.
 *
 * Without this serialization step Prisma would reject the class instance
 * as an unknown input shape.
 *
 * @example
 * ```ts
 * // Postgres
 * const prisma = new PrismaClient()
 *   .$extends(heeranjidExtension())
 *   .$extends(withAutoIds({
 *     backend: "postgres",
 *     models: { User: "heerid", Post: "ranjid" },
 *   }));
 *
 * // MSSQL — `backend` is required on BOTH extensions, and the values must match.
 * const prismaMssql = new PrismaClient()
 *   .$extends(heeranjidExtension({ backend: "mssql" }))
 *   .$extends(withAutoIds({
 *     backend: "mssql",
 *     models: { User: "heerid", Post: "ranjid" },
 *   }));
 *
 * // IDs are generated automatically and serialized to the matching
 * // wire shape for each backend:
 * await prisma.user.create({ data: { name: "Alice" } });
 * await prisma.post.createMany({ data: [{ title: "Hello" }, { title: "World" }] });
 * ```
 */
export function withAutoIds(config: AutoIdConfig) {
  // Runtime validation (C2 + V4): fail fast at construction time.
  //
  // C2 — `config.backend`: TS forces it to `"postgres" | "mssql"` at
  //   compile time, but JS callers can hand us anything and a silent
  //   fall-through to the postgres serializer on an MSSQL extension
  //   would inject UUID strings into a `BINARY(16)` column.
  //
  // V4 — `config.models[*]`: each value must be `"heerid"` or
  //   `"ranjid"`. Validating the whole map up-front (rather than
  //   per-create on first lookup) means the failure mode is a
  //   constructor `TypeError` at boot, not a partial outage after
  //   some rows have already been created with the wrong id type.
  assertBackend(config.backend, "withAutoIds");
  if (config.models === undefined || config.models === null || typeof config.models !== "object" || Array.isArray(config.models)) {
    throw new TypeError(
      `withAutoIds: models must be an object mapping model names to "heerid" or "ranjid", got ${JSON.stringify(config.models)}.`
    );
  }
  for (const [modelName, idKind] of Object.entries(config.models)) {
    assertIdKind(idKind, modelName);
  }
  const idField = config.idField ?? "id";
  const backend: Backend = config.backend;

  // Serializes a generated id class instance to the wire shape Prisma
  // accepts for the configured backend. Kept as a local closure so the
  // dispatch on `backend` happens once at config time, not per-create.
  function serializeHeerId(id: HeerId): bigint {
    return id.toBigInt();
  }
  function serializeRanjId(id: RanjId): string | Uint8Array {
    return backend === "mssql" ? id.toBytes() : id.toString();
  }

  return {
    name: "heeranjid-auto-ids",
    query: {
      $allModels: {
        async create({ model, args, query }: any) {
          if (model in config.models && !args.data?.[idField]) {
            const client: any = this;
            const heeranjid = (client.$parent ?? client).$heeranjid;
            const nodeId = config.nodeId ?? getNodeId();
            const idType = config.models[model];

            const generated =
              idType === "heerid"
                ? serializeHeerId(await heeranjid.generateHeerId(nodeId))
                : serializeRanjId(await heeranjid.generateRanjId(nodeId));

            args = {
              ...args,
              data: {
                ...args.data,
                [idField]: generated,
              },
            };
          }
          return query(args);
        },

        async createMany({ model, args, query }: any) {
          if (model in config.models) {
            const items: any[] = Array.isArray(args.data)
              ? args.data
              : [args.data];
            const missing = items.filter((item) => item?.[idField] == null);

            if (missing.length > 0) {
              const client: any = this;
              const heeranjid = (client.$parent ?? client).$heeranjid;
              const nodeId = config.nodeId ?? getNodeId();
              const idType = config.models[model];

              const generatedIds: (bigint | string | Uint8Array)[] =
                idType === "heerid"
                  ? (
                      await heeranjid.generateHeerIds(nodeId, missing.length)
                    ).map(serializeHeerId)
                  : (
                      await heeranjid.generateRanjIds(nodeId, missing.length)
                    ).map(serializeRanjId);

              let idx = 0;
              for (const item of items) {
                if (item?.[idField] == null) {
                  item[idField] = generatedIds[idx++];
                }
              }
            }
          }
          return query(args);
        },
      },
    },
  };
}
