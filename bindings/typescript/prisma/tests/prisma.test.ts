import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { heeranjidExtension, getNodeId, withAutoIds } from "../src/index.js";
import { getInstallSQL, getSeedSQL, getConfigureSQL } from "../src/setup.js";
// Pulled from the mock-aliased `heeranjid` module (see vitest.config.ts).
// `withAutoIds` now calls `.toBigInt()` / `.toString()` / `.toBytes()` on
// the values returned by `generateHeerId` / `generateRanjId`, so the
// `withAutoIds` tests must return real class instances from their mocks
// instead of raw `bigint` / `string` values. Importing the mock-aliased
// classes (rather than re-implementing them inline) keeps these tests
// honest about the production contract.
import { HeerId, RanjId } from "heeranjid";

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

    // ------------------------------------------------------------------
    // Runtime validators (V1, V2 per Codex review 2026-05)
    //
    // TS narrows `backend` at compile time but JS / `any` casts can
    // pass anything. These tests pin the runtime-throw contract so a
    // future refactor that drops the validator surfaces here.
    // ------------------------------------------------------------------
    it("heeranjidExtension rejects an invalid backend literal at runtime", () => {
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check.
        // Prisma's MSSQL provider is "sqlserver" but this extension uses "mssql";
        // a JS consumer reaching for the Prisma-native name would land here.
        heeranjidExtension({ backend: "sqlserver" })
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        heeranjidExtension({ backend: "sqlserver" })
      ).toThrow(/backend must be "postgres" or "mssql".*got "sqlserver"/);
    });

    it("heeranjidExtension accepts an omitted backend (default postgres)", () => {
      // No backend supplied → falls through to the documented default.
      // The validator only fires when an explicit value is passed.
      const ext = heeranjidExtension();
      expect(ext.name).toBe("heeranjid");
    });

    it("install() rejects an invalid backend override at runtime", async () => {
      const ext = heeranjidExtension({ backend: "postgres" });
      const mockClient = {
        $executeRawUnsafe: async (_sql: string, ..._args: unknown[]) => {},
      };
      const install = ext.client.$heeranjid.install.bind({
        $parent: mockClient,
      });
      // @ts-expect-error invalid backend literal — runtime check on
      // the install() override path. The override must be validated
      // even though the extension-level backend was valid: install()
      // is an explicit user-facing entry point and a JS caller could
      // pass the Prisma provider name directly.
      await expect(install("sqlserver")).rejects.toThrow(TypeError);
    });
  });

  // ---------------------------------------------------------------------------
  // MSSQL backend dispatch — verifies that constructing the extension with
  // backend: "mssql" routes every `$heeranjid` method through the SQL Server
  // dialect (EXEC procedure with @P1 positional params, BINARY(16) rows
  // decoded via RanjId.fromBytes). Mirrors the just-landed .NET parity fix.
  // ---------------------------------------------------------------------------
  describe("MSSQL backend dispatch", () => {
    /**
     * Build a mock Prisma client that records every SQL statement and
     * positional parameters issued via `$queryRawUnsafe` and
     * `$executeRawUnsafe`. `queryResults` is a queue of canned result sets
     * returned in order by successive `$queryRawUnsafe` calls.
     */
    function makeMockClient(queryResults: unknown[][] = []) {
      const queryCalls: { sql: string; params: unknown[] }[] = [];
      const executeCalls: { sql: string; params: unknown[] }[] = [];
      let queryIdx = 0;
      const mockClient = {
        async $queryRawUnsafe(sql: string, ...params: unknown[]) {
          queryCalls.push({ sql, params });
          return queryResults[queryIdx++] ?? [];
        },
        async $executeRawUnsafe(sql: string, ...params: unknown[]) {
          executeCalls.push({ sql, params });
        },
      };
      return { mockClient, queryCalls, executeCalls };
    }

    // Valid UUIDv8 big-endian bytes — version=8 (byte[6] high nibble = 0x80),
    // variant=10 (byte[8] high two bits = 0b10), encoding
    // timestamp=0, precision=us(0), node=100, sequence=200. Matches
    // ValidUuidBytes in the .NET RanjIdTests for cross-binding parity.
    const validRanjIdBytes = Buffer.from([
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
      0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xc8,
    ]);

    // Bare Uint8Array shape — what Prisma 6+ actually returns from the
    // sqlserver adapter for `BINARY(16)` columns (Prisma 5 used Buffer,
    // which is a Uint8Array subclass, so the existing Buffer-based
    // tests above are still relevant but no longer exercise the
    // production path). Same byte sequence as `validRanjIdBytes`.
    const validRanjIdU8 = new Uint8Array([
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
      0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xc8,
    ]);

    it("setNodeId issues EXEC heer_set_node_id @P1 with the node id parameter", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, executeCalls } = makeMockClient();
      const setNodeId = ext.client.$heeranjid.setNodeId.bind({
        $parent: mockClient,
      });

      await setNodeId(42);

      expect(executeCalls).toHaveLength(1);
      expect(executeCalls[0].sql).toBe("EXEC heer_set_node_id @P1");
      expect(executeCalls[0].params).toEqual([42]);
    });

    it("setRanjNodeId issues EXEC heer_set_ranj_node_id @P1 with the node id parameter", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, executeCalls } = makeMockClient();
      const setRanjNodeId = ext.client.$heeranjid.setRanjNodeId.bind({
        $parent: mockClient,
      });

      await setRanjNodeId(17);

      expect(executeCalls).toHaveLength(1);
      expect(executeCalls[0].sql).toBe("EXEC heer_set_ranj_node_id @P1");
      expect(executeCalls[0].params).toEqual([17]);
    });

    it("generateRanjId issues EXEC generate_ranjids and decodes BINARY(16) via fromBytes", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: validRanjIdBytes }],
      ]);
      const generateRanjId = ext.client.$heeranjid.generateRanjId.bind({
        $parent: mockClient,
      });

      const id = await generateRanjId(7);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ranjids @in_node_id = @P1, @requested_count = 1"
      );
      expect(queryCalls[0].params).toEqual([7]);
      // The mock RanjId.fromBytes hex-encodes the buffer; verify the result
      // matches the canonical UUID for these bytes.
      expect(id.toString()).toBe("00000000-0000-8000-8000-0000006400c8");
    });

    it("generateRanjIds issues EXEC generate_ranjids with both params and decodes each row via fromBytes", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      // Second row: same valid layout but sequence=201 so we know each row
      // is decoded independently.
      const secondBytes = Buffer.from([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
        0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xc9,
      ]);
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: validRanjIdBytes }, { id: secondBytes }],
      ]);
      const generateRanjIds = ext.client.$heeranjid.generateRanjIds.bind({
        $parent: mockClient,
      });

      const ids = await generateRanjIds(7, 2);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ranjids @in_node_id = @P1, @requested_count = @P2"
      );
      expect(queryCalls[0].params).toEqual([7, 2]);
      expect(ids).toHaveLength(2);
      expect(ids[0].toString()).toBe("00000000-0000-8000-8000-0000006400c8");
      expect(ids[1].toString()).toBe("00000000-0000-8000-8000-0000006400c9");
    });

    // ------------------------------------------------------------------
    // Prisma 6+ wire shape: BINARY(16) columns surface as bare
    // `Uint8Array` (NOT `Buffer`). napi-rs's `Buffer` `FromNapiValue`
    // impl rejects bare `Uint8Array` with "Expected a Buffer value", so
    // the production path was previously broken against real Prisma 6+
    // until the native `fromBytes` signature was widened to accept
    // `Uint8Array`. These tests prove that path works end-to-end with
    // the actual production wire shape, not its Buffer subclass.
    // ------------------------------------------------------------------
    it("generateRanjId accepts bare Uint8Array rows (Prisma 6+ shape) and decodes via fromBytes", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: validRanjIdU8 }],
      ]);
      const generateRanjId = ext.client.$heeranjid.generateRanjId.bind({
        $parent: mockClient,
      });

      const id = await generateRanjId(7);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ranjids @in_node_id = @P1, @requested_count = 1"
      );
      expect(queryCalls[0].params).toEqual([7]);
      expect(id.toString()).toBe("00000000-0000-8000-8000-0000006400c8");
    });

    it("generateRanjIds accepts bare Uint8Array rows (Prisma 6+ shape)", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const secondBytesU8 = new Uint8Array([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
        0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xc9,
      ]);
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: validRanjIdU8 }, { id: secondBytesU8 }],
      ]);
      const generateRanjIds = ext.client.$heeranjid.generateRanjIds.bind({
        $parent: mockClient,
      });

      const ids = await generateRanjIds(7, 2);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ranjids @in_node_id = @P1, @requested_count = @P2"
      );
      expect(queryCalls[0].params).toEqual([7, 2]);
      expect(ids).toHaveLength(2);
      expect(ids[0].toString()).toBe("00000000-0000-8000-8000-0000006400c8");
      expect(ids[1].toString()).toBe("00000000-0000-8000-8000-0000006400c9");
    });

    it("generateHeerId issues EXEC generate_ids on the MSSQL backend", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: BigInt(123) }],
      ]);
      const generateHeerId = ext.client.$heeranjid.generateHeerId.bind({
        $parent: mockClient,
      });

      const id = await generateHeerId(3);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ids @in_node_id = @P1, @requested_count = 1"
      );
      expect(queryCalls[0].params).toEqual([3]);
      expect(id.toBigInt()).toBe(BigInt(123));
    });

    it("generateHeerIds issues EXEC generate_ids with both params on the MSSQL backend", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, queryCalls } = makeMockClient([
        [{ id: BigInt(100) }, { id: BigInt(101) }, { id: BigInt(102) }],
      ]);
      const generateHeerIds = ext.client.$heeranjid.generateHeerIds.bind({
        $parent: mockClient,
      });

      const ids = await generateHeerIds(3, 3);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "EXEC generate_ids @in_node_id = @P1, @requested_count = @P2"
      );
      expect(queryCalls[0].params).toEqual([3, 3]);
      expect(ids.map((i) => i.toBigInt())).toEqual([
        BigInt(100),
        BigInt(101),
        BigInt(102),
      ]);
    });

    it("install() defaults to the extension-level backend (mssql) when called with no argument", async () => {
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, executeCalls } = makeMockClient();
      const install = ext.client.$heeranjid.install.bind({
        $parent: mockClient,
      });

      await install();

      // Final call must be EXEC heer_configure (MSSQL), not SELECT heer_configure().
      const last = executeCalls[executeCalls.length - 1];
      expect(last.sql).toBe("EXEC heer_configure");

      // Regression guard for NEW-C (GO-batch splitter):
      //   `getInstallSQL("mssql")` and `getSeedSQL("mssql")` together
      //   contain 18 `GO` batch separators (sqlcmd-only; ODBC/OLE DB
      //   does not parse `GO` as a statement). Prisma's
      //   `$executeRawUnsafe` only accepts single statements, so the
      //   install path must split on `^GO\s*$` and issue each batch as
      //   its own call. Asserting NO single `$executeRawUnsafe` call
      //   contains a line-only `GO` proves the splitter ran on every
      //   batch we emitted. If this assertion fires, the MSSQL install
      //   path silently regressed to one-shot execution.
      for (const call of executeCalls) {
        expect(call.sql).not.toMatch(/^GO\s*$/m);
      }
      // Also: the install + seed + configure flow must emit more than
      // one batch on MSSQL (each `CREATE` is its own batch in the
      // source SQL). A single-call execute would mean the splitter is
      // a no-op or the install path bypassed it entirely.
      expect(executeCalls.length).toBeGreaterThan(1);
    });

    it("install('postgres') override on an mssql-configured extension issues SELECT heer_configure()", async () => {
      // Backward-compat guarantee: install()'s explicit-arg form still wins.
      const ext = heeranjidExtension({ backend: "mssql" });
      const { mockClient, executeCalls } = makeMockClient();
      const install = ext.client.$heeranjid.install.bind({
        $parent: mockClient,
      });

      await install("postgres");

      const last = executeCalls[executeCalls.length - 1];
      expect(last.sql).toBe("SELECT heer_configure()");
    });
  });

  // ---------------------------------------------------------------------------
  // Postgres backend regression — the default backend path must keep using
  // SELECT-style SQL with $1 placeholders and the ::text cast, even after the
  // MSSQL branch was added. Guards against accidentally dropping the
  // Postgres path while wiring up MSSQL.
  // ---------------------------------------------------------------------------
  describe("Postgres backend regression", () => {
    function makeMockClient(queryResults: unknown[][] = []) {
      const queryCalls: { sql: string; params: unknown[] }[] = [];
      const executeCalls: { sql: string; params: unknown[] }[] = [];
      let queryIdx = 0;
      const mockClient = {
        async $queryRawUnsafe(sql: string, ...params: unknown[]) {
          queryCalls.push({ sql, params });
          return queryResults[queryIdx++] ?? [];
        },
        async $executeRawUnsafe(sql: string, ...params: unknown[]) {
          executeCalls.push({ sql, params });
        },
      };
      return { mockClient, queryCalls, executeCalls };
    }

    it("default extension (no options) uses the postgres ::text path for generateRanjId", async () => {
      const ext = heeranjidExtension();
      const fakeUuid = "00000000-0000-8000-8000-0000006400c8";
      const { mockClient, queryCalls } = makeMockClient([[{ id: fakeUuid }]]);
      const generateRanjId = ext.client.$heeranjid.generateRanjId.bind({
        $parent: mockClient,
      });

      const id = await generateRanjId(7);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "SELECT id::text FROM generate_ranjids($1, 1)"
      );
      expect(queryCalls[0].params).toEqual([7]);
      expect(id.toString()).toBe(fakeUuid);
    });

    it("explicit backend: 'postgres' uses the postgres ::text path for generateRanjIds", async () => {
      const ext = heeranjidExtension({ backend: "postgres" });
      const fakeUuids = [
        { id: "00000000-0000-8000-8000-0000006400c8" },
        { id: "00000000-0000-8000-8000-0000006400c9" },
      ];
      const { mockClient, queryCalls } = makeMockClient([fakeUuids]);
      const generateRanjIds = ext.client.$heeranjid.generateRanjIds.bind({
        $parent: mockClient,
      });

      const ids = await generateRanjIds(7, 2);

      expect(queryCalls).toHaveLength(1);
      expect(queryCalls[0].sql).toBe(
        "SELECT id::text FROM generate_ranjids($1, $2)"
      );
      expect(queryCalls[0].params).toEqual([7, 2]);
      expect(ids.map((i) => i.toString())).toEqual([
        fakeUuids[0].id,
        fakeUuids[1].id,
      ]);
    });

    it("default extension setNodeId uses SELECT set_heer_node_id($1) on postgres", async () => {
      const ext = heeranjidExtension();
      const { mockClient, executeCalls } = makeMockClient();
      const setNodeId = ext.client.$heeranjid.setNodeId.bind({
        $parent: mockClient,
      });

      await setNodeId(42);

      expect(executeCalls).toHaveLength(1);
      expect(executeCalls[0].sql).toBe("SELECT set_heer_node_id($1)");
      expect(executeCalls[0].params).toEqual([42]);
    });
  });

  describe("SQL setup helpers", () => {
    it("getInstallSQL('postgres') returns non-empty SQL containing schema and functions", () => {
      const sql = getInstallSQL("postgres");
      expect(sql.length).toBeGreaterThan(0);
      expect(sql).toContain("CREATE TABLE IF NOT EXISTS heer_nodes");
      expect(sql).toContain("set_heer_node_id");
      expect(sql).toContain("generate_ids");
      expect(sql).toContain("generate_ranjids");
    });

    it("getInstallSQL('postgres') includes configure function SQL", () => {
      const sql = getInstallSQL("postgres");
      expect(sql).toContain("heer_configure");
    });

    it("getSeedSQL('postgres') returns non-empty SQL with default node insert", () => {
      const sql = getSeedSQL("postgres");
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

    // ------------------------------------------------------------------
    // `backend` required enforcement on the SQL setup helpers
    //
    // v0.5.x bumped `getInstallSQL` / `getConfigureSQL` / `getSeedSQL`
    // from `(backend: Backend = "postgres")` to `(backend: Backend)`.
    // Reasoning mirrors `AutoIdConfig.backend`: a silent `"postgres"`
    // default would mean an MSSQL consumer who forgot to pass `"mssql"`
    // gets a Postgres-shaped install script that breaks against a
    // sqlserver datasource with a dialect error that does not point at
    // the misconfiguration. The `@ts-expect-error` directives below pin
    // this contract. If a future refactor re-introduces a default (e.g.
    // `backend: Backend = "postgres"` or `backend?: Backend`), each
    // `ts-expect-error` becomes unused and `tsc` fails with TS2578 — the
    // same regression signal used for the `withAutoIds.backend` guard
    // above. These guards only have teeth when CI runs
    // `npm run typecheck:tests` (see package.json + CI workflow).
    //
    // The bad calls below sit inside `if (false)` blocks so vitest
    // never executes them (they would throw at SQL-root resolution
    // anyway); only `tsc` walks them, which is exactly the surface we
    // want to guard.
    // ------------------------------------------------------------------
    it("requires `backend` at the type level on getInstallSQL (omitting it is a TS error)", () => {
      if ((false as boolean)) {
        // @ts-expect-error backend is required on getInstallSQL — omitting
        // it must fail TS compilation (TS2554). If the directive becomes
        // unused (signature relaxed back to default), tsc fails TS2578.
        getInstallSQL();
      }
      // Runtime-side sanity: the well-typed form still works.
      expect(typeof getInstallSQL).toBe("function");
    });

    it("requires `backend` at the type level on getConfigureSQL (omitting it is a TS error)", () => {
      if ((false as boolean)) {
        // @ts-expect-error backend is required on getConfigureSQL.
        getConfigureSQL();
      }
      expect(typeof getConfigureSQL).toBe("function");
    });

    it("requires `backend` at the type level on getSeedSQL (omitting it is a TS error)", () => {
      if ((false as boolean)) {
        // @ts-expect-error backend is required on getSeedSQL.
        getSeedSQL();
      }
      expect(typeof getSeedSQL).toBe("function");
    });

    // ------------------------------------------------------------------
    // Runtime validators (V3 per Codex review 2026-05)
    //
    // The three SQL helpers also validate `backend` at runtime to
    // catch JS callers passing the wrong literal (e.g. Prisma's
    // `"sqlserver"` provider name). Without these, an invalid backend
    // would fall through to `readSQL` / `readRoutineSQL`, which then
    // hardcodes `"functions"` for anything not `"mssql"` and produces
    // a confusing ENOENT-style error far from the call site.
    // ------------------------------------------------------------------
    it("getInstallSQL rejects an invalid backend literal at runtime", () => {
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getInstallSQL("sqlserver")
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getInstallSQL("sqlserver")
      ).toThrow(/getInstallSQL: backend must be "postgres" or "mssql".*got "sqlserver"/);
    });

    it("getConfigureSQL rejects an invalid backend literal at runtime", () => {
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getConfigureSQL("postgresql")
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getConfigureSQL("postgresql")
      ).toThrow(/getConfigureSQL: backend must be "postgres" or "mssql".*got "postgresql"/);
    });

    it("getSeedSQL rejects an invalid backend literal at runtime", () => {
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getSeedSQL("oracle")
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        getSeedSQL("oracle")
      ).toThrow(/getSeedSQL: backend must be "postgres" or "mssql".*got "oracle"/);
    });
  });

  describe("withAutoIds", () => {
    it("returns a valid Prisma extension shape with query component", () => {
      const ext = withAutoIds({ backend: "postgres", models: { User: "heerid" } });
      expect(ext.name).toBe("heeranjid-auto-ids");
      expect(ext.query).toBeDefined();
      expect(ext.query.$allModels).toBeDefined();
      expect(typeof ext.query.$allModels.create).toBe("function");
      expect(typeof ext.query.$allModels.createMany).toBe("function");
    });

    it("passes through create when model is not in config", async () => {
      const ext = withAutoIds({ backend: "postgres", models: { User: "heerid" } });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { id: BigInt(1), name: "Alice" };
      };

      // Returning a `HeerId` here would never be reached (model "Post"
      // is not in the config map), but we still return a real class
      // instance so this mock matches the production contract.
      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateHeerId: async () => HeerId.fromBigInt(BigInt(99)),
            },
          },
        },
        { model: "Post", args: { data: { name: "Alice" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBeUndefined();
    });

    it("injects a HeerId into create args when model is configured and id is missing", async () => {
      // Postgres backend: HeerId values are serialized via `.toBigInt()`
      // before injection, so the captured `data.id` is a bare `bigint` —
      // Prisma's `bigint` column type accepts that directly on both
      // Postgres and MSSQL.
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
        models: { User: "heerid" },
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { id: BigInt(42), name: "Alice" };
      };

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              // Real production return shape: a `HeerId` class instance
              // (not a bare bigint).
              generateHeerId: async (_nodeId: number) =>
                HeerId.fromBigInt(BigInt(42)),
            },
          },
        },
        { model: "User", args: { data: { name: "Alice" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBe(BigInt(42));
    });

    it("injects a RanjId as a UUID string on the postgres backend", async () => {
      // Postgres backend: RanjId values are serialized via `.toString()`
      // (canonical hyphenated UUID), which Prisma's `uuid` column accepts.
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
        models: { Post: "ranjid" },
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return args;
      };
      const fakeUuid = "00000000-0000-8000-8000-000000000001";

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              // Real production return shape: a `RanjId` class instance.
              generateRanjId: async (_nodeId: number) =>
                RanjId.fromString(fakeUuid),
            },
          },
        },
        { model: "Post", args: { data: { title: "Hello" } }, query: mockQuery }
      );

      expect(capturedArgs[0].data.id).toBe(fakeUuid);
      // Sanity: ensure the captured shape is exactly a string, not a
      // `RanjId` instance (Prisma would reject a class instance).
      expect(typeof capturedArgs[0].data.id).toBe("string");
    });

    it("injects a RanjId as a Uint8Array on the mssql backend", async () => {
      // MSSQL backend: RanjId values are serialized via `.toBytes()` —
      // a 16-byte `Uint8Array` matching the `BINARY(16)` column wire
      // shape that Prisma's sqlserver driver expects.
      const ext = withAutoIds({
        nodeId: 1,
        backend: "mssql",
        models: { Post: "ranjid" },
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return args;
      };
      const fakeUuid = "00000000-0000-8000-8000-000000000001";

      await ext.query.$allModels.create.call(
        {
          $parent: {
            $heeranjid: {
              generateRanjId: async (_nodeId: number) =>
                RanjId.fromString(fakeUuid),
            },
          },
        },
        { model: "Post", args: { data: { title: "Hello" } }, query: mockQuery }
      );

      const injected = capturedArgs[0].data.id;
      expect(injected).toBeInstanceOf(Uint8Array);
      expect(injected.length).toBe(16);
      // Last 4 bytes encode sequence=1 in the embedded layout of the
      // canonical UUID above; we mainly care that the bytes round-trip
      // back to the same UUID string via the mock decoder.
      const roundTripped = RanjId.fromBytes(injected).toString();
      expect(roundTripped).toBe(fakeUuid);
    });

    it("does not overwrite an existing id in create", async () => {
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
        models: { User: "heerid" },
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
              generateHeerId: async () => HeerId.fromBigInt(BigInt(999)),
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
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
        models: { User: "heerid" },
      });
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
              // Real production return shape: an array of `HeerId`
              // instances, not bare bigints.
              generateHeerIds: async (_nodeId: number, count: number) =>
                Array.from({ length: count }, (_, i) =>
                  HeerId.fromBigInt(BigInt(100 + i))
                ),
            },
          },
        },
        { model: "User", args: { data: items }, query: mockQuery }
      );

      expect(capturedArgs[0].data[0].id).toBe(BigInt(100));
      expect(capturedArgs[0].data[1].id).toBe(BigInt(5));  // untouched
      expect(capturedArgs[0].data[2].id).toBe(BigInt(101));
    });

    it("injects RanjIds as canonical UUID strings into createMany items missing ids on the postgres backend", async () => {
      // Completes the `(idType x backend x op)` matrix: `create+ranjid+postgres`
      // already validates the string-serializer path on the single-row
      // operation; this test covers the same `serializeRanjId` closure on
      // the multi-row `createMany` path. Regression risk is low (same
      // closure feeds both), but the matrix should be complete: any
      // future refactor that special-cases createMany — e.g. inlining
      // serialization rather than reusing the shared closure — would
      // silently skip the postgres serializer on the multi-row path
      // without this test firing.
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
        models: { Post: "ranjid" },
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { count: 3 };
      };

      // Three distinct canonical UUIDv8 strings. Middle row has a pre-set
      // id to verify the injector preserves caller-supplied values on
      // createMany — parity with the `createMany + ranjid + mssql` test
      // below. Pre-set value is a *string* (postgres wire shape) so the
      // captured `data[1].id` round-trips back to the same string.
      const preSetUuid = "00000000-0000-8000-8000-000000000005";
      const generatedUuids = [
        "00000000-0000-8000-8000-000000000064",
        "00000000-0000-8000-8000-000000000065",
      ];
      const items = [
        { title: "Hello" },
        { id: preSetUuid, title: "World" },
        { title: "Sailor" },
      ];

      await ext.query.$allModels.createMany.call(
        {
          $parent: {
            $heeranjid: {
              // Real production return shape: an array of `RanjId`
              // class instances. The `withAutoIds` createMany path maps
              // each through `serializeRanjId`, which on postgres
              // resolves to `.toString()` (canonical hyphenated UUID).
              generateRanjIds: async (_nodeId: number, count: number) => {
                // Defensive: only two items are missing ids, so the
                // production code must request exactly that many.
                expect(count).toBe(2);
                return generatedUuids
                  .slice(0, count)
                  .map((u) => RanjId.fromString(u));
              },
            },
          },
        },
        { model: "Post", args: { data: items }, query: mockQuery }
      );

      const captured = capturedArgs[0].data;
      expect(captured).toHaveLength(items.length);
      // Each generated row's id must be a bare `string` — the exact wire
      // shape Prisma writes into a postgres `uuid` column. A
      // `Uint8Array` injection here would mean the postgres createMany
      // path silently regressed to the MSSQL serializer.
      expect(typeof captured[0].id).toBe("string");
      expect(typeof captured[2].id).toBe("string");
      expect(captured[0].id).toBe(generatedUuids[0]);
      expect(captured[2].id).toBe(generatedUuids[1]);
      // Canonical 8-4-4-4-12 hyphenated form, length 36.
      expect((captured[0].id as string).length).toBe(36);
      expect((captured[2].id as string).length).toBe(36);
      // Each generated id must parse cleanly through `RanjId.fromString`
      // — proves the value is well-formed canonical UUIDv8 (version +
      // variant nibbles intact) and not just a stringified instance.
      expect(RanjId.fromString(captured[0].id).toString()).toBe(generatedUuids[0]);
      expect(RanjId.fromString(captured[2].id).toString()).toBe(generatedUuids[1]);
      // The pre-set middle row must be left untouched: the injector
      // only fills slots where the id is null/undefined.
      expect(captured[1].id).toBe(preSetUuid);
    });

    it("injects RanjIds as Uint8Array bytes into createMany items missing ids on the mssql backend", async () => {
      // Mirrors the `createMany + HeerId` test above but exercises the
      // `ranjid + mssql` cell of the (idType x backend x op) grid. The
      // production path at `withAutoIds`/`createMany` for this combo
      // calls `generateRanjIds(...).map(serializeRanjId)` and on
      // `backend: "mssql"` `serializeRanjId` routes through
      // `id.toBytes()`, yielding a 16-byte `Uint8Array` per row that the
      // sqlserver adapter writes into the `BINARY(16)` column without
      // applying the Guid mixed-endian swizzle.
      const ext = withAutoIds({
        nodeId: 1,
        backend: "mssql",
        models: { Post: "ranjid" },
      });
      const capturedArgs: any[] = [];
      const mockQuery = async (args: any) => {
        capturedArgs.push(args);
        return { count: 3 };
      };

      // Three distinct canonical UUIDv8 strings (sequence varies in the
      // low byte) so we can assert per-row independence after byte
      // serialization. Middle row has a pre-set id to guard against the
      // injector clobbering caller-supplied ids on createMany — parity
      // with the HeerId variant above.
      const preSetUuid = "00000000-0000-8000-8000-000000000005";
      const generatedUuids = [
        "00000000-0000-8000-8000-000000000064",
        "00000000-0000-8000-8000-000000000065",
      ];
      const items = [
        { title: "Hello" },
        { id: RanjId.fromString(preSetUuid).toBytes(), title: "World" },
        { title: "Sailor" },
      ];

      await ext.query.$allModels.createMany.call(
        {
          $parent: {
            $heeranjid: {
              // Real production return shape: an array of `RanjId`
              // class instances. The `withAutoIds` createMany path
              // maps each through `serializeRanjId`, which on MSSQL
              // resolves to `.toBytes()`.
              generateRanjIds: async (_nodeId: number, count: number) => {
                // Defensive: the production code requests exactly
                // `missing.length` ids. Two items are missing ids in
                // our input, so we expect count === 2.
                expect(count).toBe(2);
                return generatedUuids
                  .slice(0, count)
                  .map((u) => RanjId.fromString(u));
              },
            },
          },
        },
        { model: "Post", args: { data: items }, query: mockQuery }
      );

      const captured = capturedArgs[0].data;
      expect(captured).toHaveLength(items.length);
      // Each row's id must be a 16-byte `Uint8Array` — that is the
      // exact wire shape Prisma's sqlserver driver writes into a
      // `BINARY(16)` column. A bare-string injection here would mean
      // the MSSQL createMany path silently regressed to the postgres
      // serializer and would surface as a runtime type error against
      // a real sqlserver datasource.
      for (const row of captured) {
        expect(row.id).toBeInstanceOf(Uint8Array);
        expect(row.id.length).toBe(16);
      }
      // Generated rows round-trip back through `RanjId.fromBytes` to
      // the same canonical UUID — validates that the bytes encode a
      // well-formed UUIDv8 with the version + variant nibbles intact
      // (i.e. `toBytes()` did not corrupt them).
      expect(RanjId.fromBytes(captured[0].id).toString()).toBe(generatedUuids[0]);
      expect(RanjId.fromBytes(captured[2].id).toString()).toBe(generatedUuids[1]);
      // The pre-set middle row must be left untouched: the injector
      // only fills slots where the id is null/undefined.
      expect(RanjId.fromBytes(captured[1].id).toString()).toBe(preSetUuid);
    });

    it("respects custom idField in config", async () => {
      const ext = withAutoIds({
        nodeId: 1,
        backend: "postgres",
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
              generateHeerId: async () => HeerId.fromBigInt(BigInt(77)),
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

    // ------------------------------------------------------------------
    // `backend` required enforcement — TWO LAYERS
    //
    // Mismatched `backend` between `heeranjidExtension` and `withAutoIds`
    // is a footgun: a default `"postgres"` on `withAutoIds` would
    // serialize RanjIds as UUID strings into a `BINARY(16)` column,
    // and the sqlserver driver would reject the insert with an error
    // that does not point at the misconfiguration.
    //
    // We now enforce this on **two** layers (was previously TS-only):
    //
    //   1. **Compile time** — `AutoIdConfig.backend` is a required
    //      property (no `?`, no default). The `@ts-expect-error`
    //      directive below pins this: if a future refactor relaxes
    //      `backend` back to optional, the directive becomes unused
    //      and `tsc` fails with TS2578.
    //
    //   2. **Runtime** (added 2026-05 per Codex review V4/C2) —
    //      `withAutoIds` calls `assertBackend(config.backend, ...)` at
    //      the top, so JS callers that bypass TS (or `any`-cast
    //      consumers) get a clear `TypeError` instead of a silent
    //      postgres-defaulted execution. Validates compile-time-only
    //      contract drift caught only at runtime by `any` / JS callers.
    // ------------------------------------------------------------------
    it("requires `backend` at the type level (omitting it is a TS error AND a runtime throw)", () => {
      expect(() => {
        // @ts-expect-error backend is required on AutoIdConfig —
        // omitting it must fail TS compilation. If this directive
        // becomes unused (i.e. `backend` is relaxed back to optional),
        // `tsc` fails with TS2578 and the build breaks — the desired
        // signal for a contract regression.
        withAutoIds({ models: { User: "heerid" } });
      }).toThrow(TypeError);
      expect(() => {
        // Second call to capture the message shape — the validator
        // must mention both expected values and the offending input
        // so a JS consumer sees a clear diagnosis instead of a silent
        // postgres-default that surfaces only at first `create()`.
        // @ts-expect-error backend is required on AutoIdConfig.
        withAutoIds({ models: { User: "heerid" } });
      }).toThrow(/backend must be "postgres" or "mssql"/);
    });

    // ------------------------------------------------------------------
    // Runtime validators (V1-V4, C2 per Codex review 2026-05)
    //
    // TypeScript narrows `Backend` / `idKind` at compile time, but JS
    // callers and `any`-cast consumers can pass anything and a silent
    // fall-through to a default branch would emit wrong SQL or the
    // wrong wire shape. These tests exercise each public entry point
    // that accepts a `Backend` or `idKind` and verifies the runtime
    // validator throws a `TypeError` with a useful message.
    // ------------------------------------------------------------------
    it("withAutoIds rejects invalid backend at runtime with a clear TypeError", () => {
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        withAutoIds({ backend: "sqlserver", models: { User: "heerid" } })
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid backend literal — runtime check
        withAutoIds({ backend: "sqlserver", models: { User: "heerid" } })
      ).toThrow(/backend must be "postgres" or "mssql".*got "sqlserver"/);
    });

    it("withAutoIds rejects invalid idKind values in models map", () => {
      expect(() =>
        // @ts-expect-error invalid idKind literal — runtime check
        withAutoIds({ backend: "postgres", models: { User: "snowflake" } })
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error invalid idKind literal — runtime check
        withAutoIds({ backend: "postgres", models: { User: "snowflake" } })
      ).toThrow(/withAutoIds models\.User: idKind must be "heerid" or "ranjid".*got "snowflake"/);
    });

    it("withAutoIds rejects a missing models map at runtime", () => {
      expect(() =>
        // @ts-expect-error models is required — runtime check
        withAutoIds({ backend: "postgres" })
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error models is required — runtime check
        withAutoIds({ backend: "postgres" })
      ).toThrow(/models must be an object/);
    });

    it("withAutoIds rejects an array passed as models (typeof [] === 'object' bypasses naive type check)", () => {
      // `typeof []` is `"object"`, so without an explicit `Array.isArray`
      // check an array would slip past the `typeof !== "object"` guard.
      // The consequence is silent: `Object.entries([])` on a sparse array
      // returns index strings (`"0"`, `"1"`, …) as model names, which
      // would look up valid models like `"0"` — yielding no match but
      // also no error, making the misconfiguration invisible at boot.
      expect(() =>
        // @ts-expect-error array is not a valid AutoIdModelMap — runtime check
        withAutoIds({ backend: "postgres", models: [] })
      ).toThrow(TypeError);
      expect(() =>
        // @ts-expect-error array is not a valid AutoIdModelMap — runtime check
        withAutoIds({ backend: "postgres", models: [] })
      ).toThrow(/models must be an object/);
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

  // ---------------------------------------------------------------------------
  // Mock `RanjId.fromString` validation parity (V5 per Codex review 2026-05)
  //
  // The lightweight `tests/__mocks__/heeranjid.ts` stub is used by every
  // `vitest` test in this file (see vitest.config.ts `alias: { heeranjid:
  // ... }`). Until the V5 fix, the mock's `fromString` accepted any
  // string without validation, which meant mock-based tests could pass
  // on inputs the real NAPI binding rejects. That's the same
  // hollow-test class as a prior incident.
  //
  // The fix widens the mock to mirror `heeranjid::RanjId::from_uuid`
  // validation (length, hyphens, hex chars, UUIDv8 version nibble,
  // RFC 4122 variant). The tests below pin that contract by feeding
  // inputs the real binding rejects and asserting the mock rejects
  // them too with parity error messages.
  // ---------------------------------------------------------------------------
  describe("Mock RanjId.fromString validation parity", () => {
    it("rejects a non-UUID garbage string", () => {
      expect(() => RanjId.fromString("not-a-ranjid")).toThrow(
        /invalid RanjId string/
      );
    });

    it("rejects a UUID with wrong length", () => {
      expect(() => RanjId.fromString("too-short")).toThrow(
        /invalid RanjId string/
      );
      // 35 chars (one short) — would slip past a naive length-by-byte check
      // that didn't compare exactly to 36.
      expect(() =>
        RanjId.fromString("00000000-0000-8000-8000-00000000000")
      ).toThrow(/invalid RanjId string/);
    });

    it("rejects a UUID with hyphens in wrong positions", () => {
      // Single bad character at position 8 — should be '-'.
      expect(() =>
        RanjId.fromString("000000000-000-8000-8000-0000006400c8")
      ).toThrow(/invalid RanjId string/);
    });

    it("rejects a UUID with non-hex characters", () => {
      expect(() =>
        RanjId.fromString("00000000-0000-8000-8000-0000006400cZ")
      ).toThrow(/invalid RanjId string/);
    });

    it("rejects a UUIDv4 (wrong version nibble)", () => {
      // Same overall shape, but version nibble is '4' instead of '8'.
      // Mirrors the native `heeranjid::Error::InvalidRanjIdVersion`.
      expect(() =>
        RanjId.fromString("550e8400-e29b-41d4-a716-446655440000")
      ).toThrow(/uuid version must be 8/);
    });

    it("rejects a UUID with the legacy NCS variant", () => {
      // Variant nibble is '4' (binary 0100) — high two bits 0b01, the
      // "reserved for legacy NCS" variant. RFC 4122 requires 0b10.
      // Version nibble is kept at '8' so we hit the variant check, not
      // the version check.
      expect(() =>
        RanjId.fromString("00000000-0000-8000-4000-0000006400c8")
      ).toThrow(/uuid variant must be RFC 4122/);
    });

    it("accepts a well-formed UUIDv8 with RFC 4122 variant", () => {
      // Same canonical fixture used by the byte-level tests above.
      const uuid = "00000000-0000-8000-8000-0000006400c8";
      const id = RanjId.fromString(uuid);
      expect(id.toString()).toBe(uuid);
    });

    it("normalizes case to canonical lowercase", () => {
      // Real `Uuid::parse_str` accepts upper-case; canonical Display
      // is lowercase. Mock mirrors that.
      const upper = "00000000-0000-8000-8000-0000006400C8";
      const lower = "00000000-0000-8000-8000-0000006400c8";
      const id = RanjId.fromString(upper);
      expect(id.toString()).toBe(lower);
    });
  });
});
