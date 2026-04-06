import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { heeranjidExtension, getNodeId, withAutoIds } from "../src/index.js";
import { getInstallSQL, getSeedSQL, getConfigureSQL } from "../src/setup.js";

describe("Prisma extension", () => {
  describe("heeranjidExtension", () => {
    it("returns a valid Prisma extension shape", () => {
      const ext = heeranjidExtension();
      expect(ext.name).toBe("heeranjid");
      expect(ext.client).toBeDefined();
      expect(ext.client.$heeranjid).toBeDefined();
    });

    it("exposes all expected methods", () => {
      const ext = heeranjidExtension();
      const methods = ext.client.$heeranjid;
      expect(typeof methods.setNodeId).toBe("function");
      expect(typeof methods.setRanjNodeId).toBe("function");
      expect(typeof methods.generateHeerId).toBe("function");
      expect(typeof methods.generateHeerIds).toBe("function");
      expect(typeof methods.generateRanjId).toBe("function");
      expect(typeof methods.generateRanjIds).toBe("function");
    });

    it("exposes install() method", () => {
      const ext = heeranjidExtension();
      const methods = ext.client.$heeranjid;
      expect(typeof methods.install).toBe("function");
    });

    it("install() returns a Promise (thenable)", () => {
      const ext = heeranjidExtension();
      // install() requires a real DB connection to resolve; we only verify the
      // return type by calling it with a mock client-like context.
      const mockClient = {
        $executeRawUnsafe: async (_sql: string, ..._args: unknown[]) => {},
      };
      // Bind a fake `this` that has $parent pointing to our mock
      const installFn = ext.client.$heeranjid.install.bind({
        $parent: mockClient,
      });
      const result = installFn("postgres");
      expect(result).toBeInstanceOf(Promise);
      // Consume the promise to avoid unhandled rejection noise
      return result;
    });
  });

  describe("SQL setup helpers", () => {
    it("getInstallSQL returns non-empty SQL containing schema and functions", () => {
      const sql = getInstallSQL();
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("CREATE TABLE IF NOT EXISTS heer_nodes");
      expect(sql).toContain("set_heer_node_id");
      expect(sql).toContain("generate_ids");
      expect(sql).toContain("generate_ranjids");
    });

    it("getInstallSQL includes configure function SQL", () => {
      const sql = getInstallSQL();
      expect(sql).toContain("heer_configure");
    });

    it("getSeedSQL returns non-empty SQL with default node insert", () => {
      const sql = getSeedSQL();
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("INSERT INTO heer_nodes");
      expect(sql).toContain("ON CONFLICT");
    });

    it("getConfigureSQL returns non-empty SQL for postgres", () => {
      const sql = getConfigureSQL("postgres");
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("heer_configure");
    });

    it("getConfigureSQL returns non-empty SQL for mssql", () => {
      const sql = getConfigureSQL("mssql");
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("heer_configure");
    });
  });

  describe("withAutoIds", () => {
    it("returns a valid Prisma extension shape with query component", () => {
      const ext = withAutoIds({ models: { User: "heerid" } });
      expect(ext.name).toBe("heeranjid-auto-ids");
      expect(ext.query).toBeDefined();
      expect(ext.query.$allModels).toBeDefined();
      expect(typeof ext.query.$allModels.create).toBe("function");
      expect(typeof ext.query.$allModels.createMany).toBe("function");
    });

    it("passes through create when model is not in config", async () => {
      const ext = withAutoIds({ models: { User: "heerid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { id: BigInt(1), name: "Alice" };
      };

      await ext.query.$allModels.create.call(
        { $parent: { $heeranjid: { generateHeerId: async () => BigInt(99) } } },
        { model: "Post", args: { data: { name: "Alice" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBeUndefined();
    });

    it("injects a HeerId into create args when model is configured and id is missing", async () => {
      const ext = withAutoIds({ nodeId: 1, models: { User: "heerid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { id: BigInt(42), name: "Alice" };
      };

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateHeerId: async (_nodeId: number) => BigInt(42),
            },
          },
        },
        { model: "User", args: { data: { name: "Alice" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBe(BigInt(42));
    });

    it("injects a RanjId into create args when model is configured as ranjid", async () => {
      const ext = withAutoIds({ nodeId: 1, models: { Post: "ranjid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return args;
      };
      const fakeRanjId = "00000000-0000-8000-8000-000000000001";

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateRanjId: async (_nodeId: number) => fakeRanjId,
            },
          },
        },
        { model: "Post", args: { data: { title: "Hello" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBe(fakeRanjId);
    });

    it("does not overwrite an existing id in create", async () => {
      const ext = withAutoIds({ nodeId: 1, models: { User: "heerid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return args;
      };

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateHeerId: async () => BigInt(999),
            },
          },
        },
        {
          model: "User",
          args: { data: { id: BigInt(7), name: "Bob" } },
          query: mockQuery,
        }
      );

      expect(capturedArgs[0].data.id).toBe(BigInt(7));
    });

    it("injects HeerIds into createMany items missing ids", async () => {
      const ext = withAutoIds({ nodeId: 1, models: { User: "heerid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { count: 3 };
      };

      const items = [
        { name: "Alice" },
        { id: BigInt(5), name: "Bob" },
        { name: "Charlie" },
      ];

      await ext.query.$allModels.createMany.call(
        {
          $parent: {
            $heeranjid: {
              generateHeerIds: async (_nodeId: number, count: number) =>
                Array.from({ length: count }, (_, i) => BigInt(100 + i)),
            },
          },
        },
        { model: "User", args: { data: items }, query: mockQuery }
      );

      expect(capturedArgs[0].data[0].id).toBe(BigInt(100));
      expect(capturedArgs[0].data[1].id).toBe(BigInt(5));  // untouched
      expect(capturedArgs[0].data[2].id).toBe(BigInt(101));
    });

    it("respects custom idField in config", async () => {
      const ext = withAutoIds({
        nodeId: 1,
        models: { Widget: "heerid" },
        idField: "widgetId",
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return args;
      };

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateHeerId: async () => BigInt(77),
            },
          },
        },
        {
          model: "Widget",
          args: { data: { label: "Foo" } },
          query: mockQuery,
        }
      );

      expect(capturedArgs[0].data.widgetId).toBe(BigInt(77));
    });
  });

  describe("NODE_ID environment variable", () => {
    let originalNodeId: string | undefined;

    beforeEach(() => {
      originalNodeId = process.env.NODE_ID;
    });

    afterEach(() => {
      if (originalNodeId === undefined) {
        delete process.env.NODE_ID;
      } else {
        process.env.NODE_ID = originalNodeId;
      }
    });

    it("getNodeId() returns the parsed integer when NODE_ID is set", () => {
      process.env.NODE_ID = "42";
      const id = getNodeId();
      expect(id).toBe(42);
    });

    it("getNodeId() throws when NODE_ID is not set", () => {
      delete process.env.NODE_ID;
      expect(() => getNodeId()).toThrow(
        "NODE_ID environment variable must be set"
      );
    });

    it("getNodeId() returns a number type", () => {
      process.env.NODE_ID = "7";
      expect(typeof getNodeId()).toBe("number");
    });
  });
});
