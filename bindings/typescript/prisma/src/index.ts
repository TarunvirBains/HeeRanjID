import { HeerId, RanjId } from "heeranjid";
import { getInstallSQL, getSeedSQL, getConfigureSQL } from "./setup.js";

/**
 * Shape expected from Prisma's `$queryRaw` for a single HeerId row.
 */
interface HeerIdRow {
  id: bigint;
}

/**
 * Shape expected from Prisma's `$queryRaw` for a single RanjId row.
 */
interface RanjIdRow {
  id: string;
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
     * @param backend - Database backend. Defaults to "postgres".
     */
    install(backend?: "postgres" | "mssql"): Promise<void>;
  };
}

/**
 * Creates a Prisma client extension that adds `$heeranjid` methods for
 * distributed ID generation.
 *
 * Usage:
 * ```ts
 * import { PrismaClient } from "@prisma/client";
 * import { heeranjidExtension } from "heeranjid-prisma";
 *
 * const prisma = new PrismaClient().$extends(heeranjidExtension());
 *
 * const id = await prisma.$heeranjid.generateHeerId(1);
 * ```
 */
export function heeranjidExtension() {
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
          await client.$executeRawUnsafe(
            `SELECT set_heer_node_id($1)`,
            nodeId
          );
        },

        async setRanjNodeId(nodeId: number): Promise<void> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          await client.$executeRawUnsafe(
            `SELECT set_heer_ranj_node_id($1)`,
            nodeId
          );
        },

        async generateHeerId(nodeId?: number): Promise<HeerId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const resolvedNodeId = nodeId ?? getNodeId();
          const rows: HeerIdRow[] = await client.$queryRawUnsafe(
            `SELECT id FROM generate_ids($1, 1)`,
            resolvedNodeId
          );
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
          const rows: HeerIdRow[] = await client.$queryRawUnsafe(
            `SELECT id FROM generate_ids($1, $2)`,
            nodeId,
            count
          );
          return rows.map((row) => HeerId.fromBigInt(row.id));
        },

        async generateRanjId(nodeId?: number): Promise<RanjId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const resolvedNodeId = nodeId ?? getNodeId();
          const rows: RanjIdRow[] = await client.$queryRawUnsafe(
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
          const rows: RanjIdRow[] = await client.$queryRawUnsafe(
            `SELECT id::text FROM generate_ranjids($1, $2)`,
            nodeId,
            count
          );
          return rows.map((row) => RanjId.fromString(row.id));
        },

        async install(backend: "postgres" | "mssql" = "postgres"): Promise<void> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;

          const installSQL = getInstallSQL(backend);
          const seedSQL = getSeedSQL(backend);

          // Execute the full install SQL (schema + session + generate functions + configure function)
          await client.$executeRawUnsafe(installSQL);

          // Seed default node
          await client.$executeRawUnsafe(seedSQL);

          // Call heer_configure() to bake in epoch/precision and regenerate ID functions
          if (backend === "mssql") {
            await client.$executeRawUnsafe(`EXEC heer_configure`);
          } else {
            await client.$executeRawUnsafe(`SELECT heer_configure()`);
          }
        },
      },
    },
  };
}

export { getNodeId, getInstallSQL, getSeedSQL, getConfigureSQL };
