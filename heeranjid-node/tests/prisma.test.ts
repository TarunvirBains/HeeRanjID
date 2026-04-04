import { describe, it, expect } from "vitest";
import { heeranjidExtension } from "../js/prisma/index.js";
import { getInstallSQL, getSeedSQL } from "../js/prisma/setup.js";

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

    it("getSeedSQL returns non-empty SQL with default node insert", () => {
      const sql = getSeedSQL();
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("INSERT INTO heer_nodes");
      expect(sql).toContain("ON CONFLICT");
    });
  });
});
