import { HeerId, RanjId } from "../../index.js";

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
     * Generate a single HeerId.
     */
    generateHeerId(nodeId: number): Promise<HeerId>;

    /**
     * Generate multiple HeerIds.
     */
    generateHeerIds(nodeId: number, count: number): Promise<HeerId[]>;

    /**
     * Generate a single RanjId.
     */
    generateRanjId(nodeId: number): Promise<RanjId>;

    /**
     * Generate multiple RanjIds.
     */
    generateRanjIds(nodeId: number, count: number): Promise<RanjId[]>;
  };
}

/**
 * Creates a Prisma client extension that adds `$heeranjid` methods for
 * distributed ID generation.
 *
 * Usage:
 * ```ts
 * import { PrismaClient } from "@prisma/client";
 * import { heeranjidExtension } from "heeranjid";
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

        async generateHeerId(nodeId: number): Promise<HeerId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const rows: HeerIdRow[] = await client.$queryRawUnsafe(
            `SELECT id FROM generate_ids($1, 1)`,
            nodeId
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

        async generateRanjId(nodeId: number): Promise<RanjId> {
          const ctx = this as any;
          const client = ctx.$parent ?? ctx;
          const rows: RanjIdRow[] = await client.$queryRawUnsafe(
            `SELECT id::text FROM generate_ranjids($1, 1)`,
            nodeId
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
      },
    },
  };
}
