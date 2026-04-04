# Implementation Plan

## Goal

Build the full HeerRanjId system described in [README.md](./README.md), with
shared SQL in [sql](./sql), a Postgres-first `sqlx` crate in [src](./src), and
thorough testing against a live Postgres instance.

Work should continue in atomic commits, aiming for fewer than 7 files per
commit when practical.

## Phase 1: Stabilize Current Foundation ✓

1. ~~Tighten the current API surface in [src/lib.rs](./src/lib.rs).~~
2. ~~Add missing docs and comments around exported types and SQL installer
   constants.~~
3. ~~Add boundary tests for max timestamp, max node ID, max sequence, and parse
   failures for both ID types.~~
4. ~~Add explicit tests for string deserialization and `sqlx` round-tripping of
   `HeerId` and `RanjId`.~~

## Phase 2: Complete Shared HeerId SQL ✓

1. ~~Refine the SQL install layout under [sql/postgres](./sql/postgres).~~
2. ~~Add SQL files for:~~
   - ~~bootstrap and seed helpers~~
   - ~~default node registration~~
   - ~~epoch initialization~~
   - ~~optional convenience views or comments~~
3. ~~Implement SQL defaults and adoption helpers for using `generate_id()` in
   table schemas.~~
4. ~~Add tests for:~~
   - ~~`generate_id(node_id)`~~
   - ~~`generate_id()` after `set_heer_node_id`~~
   - ~~`generate_ids(count, allow_spanning)`~~
   - ~~batch ordering~~
   - ~~exact-count guarantees~~
   - ~~non-spanning failure behavior~~
   - ~~single-update state behavior~~
   - ~~node validation failures~~
   - ~~missing epoch failure~~
   - ~~missing session node failure~~

## Phase 3: Startup Validation and Operational Helpers ✓

1. ~~Add Rust-side Postgres helpers for:~~
   - ~~fetching and validating active node config~~
   - ~~fetching epoch~~
   - ~~checking `heer_nodes` membership~~
   - ~~bootstrapping node state rows safely~~
2. ~~Add data structs for registry and config state.~~
3. ~~Add integration tests for:~~
   - ~~valid startup path~~
   - ~~inactive node rejection~~
   - ~~unknown node rejection~~
   - ~~missing `heer_config` rejection~~
4. ~~Add small SQL helpers if needed, but keep as much logic as possible in
   [sql](./sql).~~

## Phase 4: RanjId SQL Generation ✓

1. ~~Add shared SQL for RanjId generation in
   [sql/postgres/functions](./sql/postgres/functions).~~
2. ~~Implement:~~
   - ~~RanjId session-aware generation~~
   - ~~direct-node generation~~
   - ~~batch generation~~
   - ~~state updates using `heer_ranj_node_state`~~
3. ~~Ensure UUIDv7 version and variant bits are set correctly in SQL.~~
4. ~~Add Rust-side query helpers to call and parse generated UUIDs as `RanjId`.~~
5. ~~Add live integration tests for:~~
   - ~~UUID validity~~
   - ~~ordering by Time -> Node -> Sequence~~
   - ~~collision resistance within a node~~
   - ~~batch monotonicity~~
   - ~~rollback handling~~
   - ~~spanning behavior~~

## Phase 5: SQL Asset Organization for Reuse ✓

1. ~~Normalize [sql/README.md](./sql/README.md) to match the implemented SQL
   surface exactly.~~
2. ~~Split SQL into predictable categories:~~
   - ~~`schema.sql`~~
   - ~~`functions/`~~
   - ~~`queries/`~~
   - ~~optional `seed/` or `bootstrap/`~~
3. ~~Add a single install entrypoint for Postgres consumers.~~
4. ~~Make file naming and dependency order clear enough for Python or Django
   reuse.~~
5. ~~Add SQL-only smoke instructions in the SQL README.~~

## Phase 6: Rust Crate Ergonomics ✓

1. ~~Add typed wrappers for database usage:~~
   - ~~node validation APIs~~
   - ~~epoch readers~~
   - ~~generator callers~~
   - ~~optional bootstrap helpers~~
2. ~~Add feature gating if needed for future backends, while keeping Postgres as
   the default and stable backend.~~
3. ~~Add `From` and `TryFrom` conversions where useful.~~
4. ~~Add convenience methods for decoding timestamp, node, and sequence cleanly.~~
5. ~~Keep raw SQL ownership in the `sql` submodule rather than embedding logic in
   Rust strings.~~

## Phase 7: Thorough Testing ✓

1. ~~Expand unit coverage for bit packing and unpacking edge cases.~~
2. ~~Expand integration coverage for all SQL functions.~~
3. ~~Add concurrency tests against real Postgres for:~~
   - ~~parallel `generate_id`~~
   - ~~parallel `generate_ids`~~
   - ~~state correctness under contention~~
4. ~~Add rollback and overflow tests:~~
   - ~~minor rollback error~~
   - ~~major rollback error~~
   - ~~sequence exhaustion with spanning~~
   - ~~sequence exhaustion without spanning~~
5. ~~Add schema-install idempotency tests.~~
6. ~~Add tests that validate ordering semantics directly in SQL with `ORDER BY`.~~

## Phase 8: Documentation and Examples ✓

1. ~~Update top-level [README.md](./README.md) to match the implemented API.~~
2. ~~Add usage examples for:~~
   - ~~installing schema~~
   - ~~setting session node~~
   - ~~generating single IDs~~
   - ~~generating batches~~
   - ~~using default column expressions~~
3. ~~Add a short Postgres bootstrap guide using
   [scripts/postgres.sh](./scripts/postgres.sh).~~
4. ~~Keep framework-specific guidance out of the SQL submodule docs.~~

## Commit Strategy

Use small commits along these lines:

1. `Add edge-case tests for core ID types`
2. `Add Postgres bootstrap and seed SQL`
3. `Test HeerId SQL batch generation behavior`
4. `Add startup validation helpers`
5. `Implement RanjId SQL generation`
6. `Test RanjId SQL generation and ordering`
7. `Refine SQL install layout and docs`

When possible, keep each commit to:

- one logical behavior change
- one test slice proving it
- under 7 files, unless a schema + test + wiring boundary needs slightly more

## Completion Notes

All phases are complete. The following were resolved during implementation:

- ~~**Session node_id range:** Resolved. Added `set_heer_ranj_node_id()` and
  `current_heer_ranj_node_id()` for the full 0-65535 RanjId node range.~~
- ~~**BIGINT timestamp cast:** Resolved. RanjId SQL uses NUMERIC
  division/modulo for the full 2^90 range (~39.24 billion years at
  nanosecond precision).~~
- ~~**Extended epochs:** Resolved. Added `ranj_epoch_offset` column to
  `heer_config` for epochs beyond TIMESTAMP range (e.g. the Big Bang).~~

### Design Decision: Seed SQL omits epoch

`seed.sql` inserts a default node but intentionally does not set an epoch in
`heer_config`. The epoch is a deployment-specific decision (e.g. application
launch date, Unix epoch, or the Big Bang) and should be configured explicitly
by the deploying application.

## Definition of Done

The project is complete when:

- all SQL described in the spec exists in the `sql` submodule
- the Rust crate consumes that SQL rather than re-embedding logic
- HeerId and RanjId generation both work in live Postgres
- startup validation and failure modes are covered
- batch allocation semantics are tested thoroughly
- docs match implementation
- the full test suite passes against a real Postgres instance
