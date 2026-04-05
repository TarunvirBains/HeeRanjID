import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { heeranjidExtension, getNodeId } from "../src/index.js";
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
