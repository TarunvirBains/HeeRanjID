# Migration playbook: ascending to descending sort IDs

Operator-facing runbook for migrating a live table from ascending `HeerId` /
`RanjId` to their descending siblings without downtime. Synthesises spec
§5.1 and §7.1–§7.9 from
`docs/superpowers/specs/2026-04-22-descending-sort-ids-design.md`. Where this
document and the spec disagree, the spec wins.

SQL in this playbook is reproduced verbatim from the spec (which survived six
adversarial review rounds). Change only identifiers when adapting; do not
paraphrase the SQL bodies.

---

## 1. When to migrate

Migrate when one or more applies:

- **Chronological queries dominate.** Most reads are `ORDER BY id DESC LIMIT
  N` / reverse keyset pagination. The descending variant turns those into a
  natural forward scan of the primary key.
- **Dedicated DESC index exists only to accelerate those reads.** Dropping it
  reclaims storage and removes write amplification.
- **Read-path simplification.** Removing explicit `ORDER BY ... DESC`
  everywhere (especially in ORM-generated SQL) reduces query surface area.

If none apply, stay on the ascending variant.

---

## 2. Pre-flight audit

Run every item before touching the table.

- **Node id assigned.** `SELECT current_heer_node_id();` must return a value
  in range — otherwise `heerid_next_desc()` will raise post-cutover.
- **Generators reachable.** `SELECT heerid_next();` / `SELECT ranjid_next();`
  must succeed.
- **No `zzz_*` trigger already on the target table.** The autofill is named
  `zzz_<table>_autofill_desc`; a collision aborts install, and any other
  BEFORE trigger sorting after `zzz_` would run after ours and could mutate
  `NEW.id` leaving `NEW.id_desc` stale.

  ```sql
  SELECT tgname FROM pg_trigger
  WHERE tgrelid = 'tbl'::regclass AND NOT tgisinternal
  ORDER BY tgname;
  ```

- **`session_replication_role` audited.** Any session in `replica` mode
  suppresses user triggers and will leave `id_desc` NULL on its writes.
  Enumerate candidates (logical-replication apply workers, `pg_restore`) and
  plan to pause or mark the autofill `ALWAYS` for the window (§12.1).
- **No already-disabled triggers** on the migrating table:

  ```sql
  SELECT tgname, tgenabled FROM pg_trigger
  WHERE tgrelid = 'tbl'::regclass AND NOT tgisinternal AND tgenabled <> 'O';
  ```

- **Long-running transactions identified.** They will either block the
  cutover's `AccessExclusiveLock` or force it to abort on `lock_timeout`:

  ```sql
  SELECT pid, now() - xact_start AS age, state, query
  FROM pg_stat_activity
  WHERE xact_start IS NOT NULL AND now() - xact_start > interval '1 minute'
  ORDER BY age DESC;
  ```

- **PG version ≥ 13** if the table is partitioned (§9 fallback otherwise).

---

## 3. Single-table recipe (spec §7.1)

Three transaction contexts: preparation (auto-commit per statement), backfill
plus index build (procedure manages its own transactions; `CONCURRENTLY`
must be outside a transaction block), and cutover (one `BEGIN ... COMMIT`).

### 3.1 Preparation

```sql
ALTER TABLE tbl ADD COLUMN id_desc bigint;
```

Nullable, no default, no constraint — backfill hasn't run, trigger isn't
installed.

```rust
install_autofill_trigger_for_table(
    &mut conn,
    "tbl",
    &[ColumnPair { src: "id", dst: "id_desc" }],
    IdKind::Heer,
).await?;
```

Helper emits the per-table function `zzz_tbl_autofill_desc` (one INSERT and
one UPDATE branch per pair) and attaches it as `BEFORE INSERT OR UPDATE FOR
EACH ROW`. The `zzz_` prefix is load-bearing: BEFORE triggers fire in
`tgname` alphabetical order and ours must run last to see the final value
of the source column.

### 3.2 Backfill

From a non-transactional connection (procedure commits per batch):

```sql
CALL heeranjid_bulk_backfill('tbl', 'id', 'id_desc', 'heer', 10000);
```

Procedure has two loops. Fast path uses `SKIP LOCKED`; cleanup pass does
not. Both reissue `SET LOCAL lock_timeout = '30s'` per batch (tunable).
`SET LOCAL` is transaction-scoped; it resets at every `COMMIT`, so it must
be reissued each iteration. Do not set `statement_timeout` on this session.

### 3.3 Verification

For a non-nullable source (PK case):

```sql
SELECT count(*) FROM tbl WHERE id_desc IS NULL;
-- expect: 0
```

For a nullable source (FK shadow), assert the NULL-tracking invariant:

```sql
SELECT count(*) FROM tbl
WHERE (src_col IS NULL) IS DISTINCT FROM (dst_col IS NULL)
   OR (src_col IS NOT NULL AND dst_col <> heerid_to_desc(src_col));
-- expect: 0
```

Catches both missed rows and stale rows.

### 3.4 Concurrent unique index (outside any transaction)

```sql
CREATE UNIQUE INDEX CONCURRENTLY idx_tbl_id_desc ON tbl (id_desc);
```

A unique index does not prove non-nullness (Postgres permits multiple NULLs
in a UNIQUE index), so the next step cannot rely on it.

### 3.5 NOT NULL proof

Avoid the full-table scan under `AccessExclusiveLock` that a bare `SET NOT
NULL` would require:

```sql
-- Instant; no scan.
ALTER TABLE tbl ADD CONSTRAINT tbl_id_desc_nn CHECK (id_desc IS NOT NULL) NOT VALID;

-- Scans the table once, takes ShareUpdateExclusiveLock (does not block
-- normal reads/writes). Slow on large tables but non-blocking.
ALTER TABLE tbl VALIDATE CONSTRAINT tbl_id_desc_nn;

-- Fast. Postgres 12+ uses the validated CHECK as proof of non-null
-- and skips the scan under AccessExclusiveLock.
ALTER TABLE tbl ALTER COLUMN id_desc SET NOT NULL;

-- Optional: drop the redundant CHECK now that the column is NOT NULL.
ALTER TABLE tbl DROP CONSTRAINT tbl_id_desc_nn;
```

PG < 12 lacks the `SET NOT NULL` fast-path — that statement will scan under
`AccessExclusiveLock`. Upgrade or budget accordingly.

### 3.6 Cutover (one atomic transaction)

Suggested session: `SET LOCAL lock_timeout = '5s'` (fail fast on blockers),
`SET LOCAL statement_timeout = '30s'` (non-partitioned cutover is catalog
only).

```sql
BEGIN;
    ALTER TABLE tbl DROP CONSTRAINT tbl_pkey;                               -- drop old PK
    ALTER TABLE tbl ADD CONSTRAINT tbl_pkey PRIMARY KEY USING INDEX idx_tbl_id_desc;
    ALTER TABLE tbl ALTER COLUMN id_desc SET DEFAULT heerid_next_desc();
    ALTER TABLE tbl ALTER COLUMN id DROP DEFAULT;                           -- old default off
    ALTER TABLE tbl DROP COLUMN id;                                         -- old column gone
    DROP TRIGGER zzz_tbl_autofill_desc ON tbl;                              -- trigger gone
    DROP FUNCTION zzz_tbl_autofill_desc() CASCADE;                          -- its function gone
    ALTER TABLE tbl RENAME COLUMN id_desc TO id;                            -- final rename
COMMIT;
```

All statements must be one transaction — any intermediate state is
schema-inconsistent. `AccessExclusiveLock` is held briefly; all heavy work
(backfill, index build) already happened outside it.

---

## 4. Parent + child (FK) recipe (spec §7.2)

Every child with an FK to the migrating PK needs its own `_desc` column,
trigger, backfill, index, and `NOT VALID` FK.

Per child:

```sql
ALTER TABLE c ADD COLUMN p_id_desc bigint;
ALTER TABLE c
  ADD CONSTRAINT c_p_id_desc_fkey
  FOREIGN KEY (p_id_desc) REFERENCES parent(id_desc)
  NOT VALID;
```

```rust
install_autofill_trigger_for_table(
    &mut conn, "c",
    &[ColumnPair { src: "p_id", dst: "p_id_desc" }],
    IdKind::Heer,
).await?;
```

```sql
CALL heeranjid_bulk_backfill('c', 'p_id', 'p_id_desc', 'heer', 10000);
ALTER TABLE c VALIDATE CONSTRAINT c_p_id_desc_fkey;
```

The UPDATE branch of the trigger handles the cascade race: if the parent PK
changes and the child FK follows, the stale desc shadow is recomputed.

Shared cutover ordering — **one transaction across parent and every child**.
Children's FKs must never dangle, and the parent's new PK must exist before
new child FKs reference it:

```sql
BEGIN;
    -- 1. Drop every child's old FK.
    ALTER TABLE c DROP CONSTRAINT c_p_id_fkey;          -- repeat per child

    -- 2. Promote the parent (same body as §3.6).
    ALTER TABLE parent DROP CONSTRAINT parent_pkey;
    ALTER TABLE parent ADD CONSTRAINT parent_pkey PRIMARY KEY USING INDEX idx_parent_id_desc;
    ALTER TABLE parent ALTER COLUMN id_desc SET DEFAULT heerid_next_desc();
    ALTER TABLE parent ALTER COLUMN id DROP DEFAULT;
    ALTER TABLE parent DROP COLUMN id;
    DROP TRIGGER zzz_parent_autofill_desc ON parent;
    DROP FUNCTION zzz_parent_autofill_desc() CASCADE;
    ALTER TABLE parent RENAME COLUMN id_desc TO id;

    -- 3. Finalise every child.
    ALTER TABLE c DROP COLUMN p_id;
    DROP TRIGGER zzz_c_autofill_desc ON c;
    DROP FUNCTION zzz_c_autofill_desc() CASCADE;
    ALTER TABLE c RENAME COLUMN p_id_desc TO p_id;
    ALTER TABLE c
      ADD CONSTRAINT c_p_id_fkey
      FOREIGN KEY (p_id) REFERENCES parent(id);
COMMIT;
```

---

## 5. Multi-level cascade (spec §7.3)

For `P → C → GC` or fan-out `P → C1, C2, C3`: apply the parent+child
pattern recursively. Every level gets its own column, trigger, backfill,
index. The single cutover transaction touches every participating table —
fast but long-in-catalog. Raise `lock_timeout` (e.g. 30 s) for wide graphs
to tolerate brief blocker contention without aborting the whole cutover.

---

## 6. Self-FK recipe (spec §7.4) — worked example

Table `nodes(id bigint PRIMARY KEY, parent_id bigint REFERENCES nodes(id))`.
Both columns flip; one multi-pair trigger handles them.

```sql
ALTER TABLE nodes ADD COLUMN id_desc        bigint;
ALTER TABLE nodes ADD COLUMN parent_id_desc bigint;
ALTER TABLE nodes
  ADD CONSTRAINT nodes_parent_id_desc_fkey
  FOREIGN KEY (parent_id_desc) REFERENCES nodes(id_desc)
  NOT VALID;
```

```rust
install_autofill_trigger_for_table(
    &mut conn, "nodes",
    &[
        ColumnPair { src: "id",        dst: "id_desc" },
        ColumnPair { src: "parent_id", dst: "parent_id_desc" },
    ],
    IdKind::Heer,
).await?;
```

```sql
CALL heeranjid_bulk_backfill('nodes', 'id',        'id_desc',        'heer', 10000);
CALL heeranjid_bulk_backfill('nodes', 'parent_id', 'parent_id_desc', 'heer', 10000);

CREATE UNIQUE INDEX CONCURRENTLY idx_nodes_id_desc        ON nodes (id_desc);
CREATE        INDEX CONCURRENTLY idx_nodes_parent_id_desc ON nodes (parent_id_desc);

ALTER TABLE nodes VALIDATE CONSTRAINT nodes_parent_id_desc_fkey;
```

Cutover:

```sql
BEGIN;
    ALTER TABLE nodes DROP CONSTRAINT nodes_parent_id_fkey;
    ALTER TABLE nodes DROP CONSTRAINT nodes_pkey;
    ALTER TABLE nodes ADD CONSTRAINT nodes_pkey PRIMARY KEY USING INDEX idx_nodes_id_desc;
    ALTER TABLE nodes ALTER COLUMN id_desc SET DEFAULT heerid_next_desc();
    ALTER TABLE nodes ALTER COLUMN id DROP DEFAULT;
    ALTER TABLE nodes DROP COLUMN id;
    ALTER TABLE nodes DROP COLUMN parent_id;
    DROP TRIGGER zzz_nodes_autofill_desc ON nodes;
    DROP FUNCTION zzz_nodes_autofill_desc() CASCADE;
    ALTER TABLE nodes RENAME COLUMN id_desc        TO id;
    ALTER TABLE nodes RENAME COLUMN parent_id_desc TO parent_id;
    ALTER TABLE nodes
      ADD CONSTRAINT nodes_parent_id_fkey
      FOREIGN KEY (parent_id) REFERENCES nodes(id);
COMMIT;
```

---

## 7. Join tables (spec §7.5)

M:N join tables need two `_desc` columns. Two orderings:

- **Option A — single mega-transaction** alongside both parents. All three
  tables prepare, backfill, index in parallel; one cutover covers all.
- **Option B — sequential.** Migrate first parent and its FK on the join
  table; then the second parent and its FK. Smaller windows, easier to
  abort, and the trigger setup tolerates one shadow existing without the
  other between cutovers.

Install all pairs the join table carries at once (for Option A):

```rust
install_autofill_trigger_for_table(
    &mut conn, "p1_p2_join",
    &[
        ColumnPair { src: "p1_id", dst: "p1_id_desc" },
        ColumnPair { src: "p2_id", dst: "p2_id_desc" },
    ],
    IdKind::Heer,
).await?;
```

For Option B, install only the pair being migrated now and reinstall with
the full set when the second parent is ready.

---

## 8. Cycles (spec §7.4 deferred-constraints pattern)

Cycles between separate tables (`A.b_id → B.id`, `B.a_id → A.id`) require
**deferred constraints**:

```sql
ALTER TABLE a
  ADD CONSTRAINT a_b_id_desc_fkey
  FOREIGN KEY (b_id_desc) REFERENCES b(id_desc)
  DEFERRABLE INITIALLY DEFERRED
  NOT VALID;

ALTER TABLE b
  ADD CONSTRAINT b_a_id_desc_fkey
  FOREIGN KEY (a_id_desc) REFERENCES a(id_desc)
  DEFERRABLE INITIALLY DEFERRED
  NOT VALID;
```

Issue `SET CONSTRAINTS ALL DEFERRED;` as the first statement of the shared
cutover transaction so mid-transaction FK states are tolerated until
`COMMIT`. **Cycles are the most delicate case in this playbook — test in
staging with production-representative data volumes, and drill the
rollback, before attempting in production.**

---

## 9. Partitioned tables (spec §7.7)

**Warning: the partitioned-table cutover is NOT a milliseconds-class
operation and MUST be benchmarked in staging against production-
representative partition counts and data volumes.** `ALTER TABLE parent ADD
PRIMARY KEY (partition_key, id_desc)` may scan partitions and/or build
replacement indexes inside the cutover transaction. Expect seconds to tens
of minutes depending on partition count and row counts. Postgres does not
document an in-place promotion guarantee here; the step-5 partitioned
UNIQUE index reduces risk but does not eliminate partition work.

### 9.1 Supported pattern (PG 13+)

Parent partitioned by `partition_key`, PK `(partition_key, id)`:

1. Add the shadow column at the parent — propagates to every partition:

   ```sql
   ALTER TABLE parent ADD COLUMN id_desc bigint;
   ```

2. Install the autofill at the parent. PG 13+ routes inserts/updates through
   the parent's BEFORE row trigger regardless of receiving partition:

   ```rust
   install_autofill_trigger_for_table(
       &mut conn, "parent",
       &[ColumnPair { src: "id", dst: "id_desc" }],
       IdKind::Heer,
   ).await?;
   ```

3. Backfill **per leaf partition** — the procedure needs the leaf's physical
   name. Each call is idempotent under `WHERE dst_col IS NULL`:

   ```sql
   CALL heeranjid_bulk_backfill('parent_p_2026_01', 'id', 'id_desc', 'heer', 10000);
   -- one CALL per leaf partition
   ```

4. Verify at the parent (aggregates across partitions):

   ```sql
   SELECT count(*) FROM parent WHERE id_desc IS NULL;
   -- expect: 0
   ```

5. Per-partition unique indexes + parent attach. Partitioned unique indexes
   cannot be built `CONCURRENTLY` at the parent level. The parent placeholder
   must be `UNIQUE` — matching the per-partition indexes exactly is required
   for `ATTACH PARTITION` to succeed:

   ```sql
   -- Parent-level UNIQUE placeholder; starts invalid (ON ONLY leaves children
   -- unindexed). Matches the children's uniqueness exactly so ATTACH works.
   CREATE UNIQUE INDEX parent_partition_key_id_desc_idx
       ON ONLY parent (partition_key, id_desc);

   -- Per partition: concurrent, non-blocking unique-index build.
   CREATE UNIQUE INDEX CONCURRENTLY p_i_partition_key_id_desc_idx
       ON p_i (partition_key, id_desc);

   -- Per partition: catalog-only attach; fast. Once every partition is
   -- attached, parent_partition_key_id_desc_idx automatically transitions to valid.
   ALTER INDEX parent_partition_key_id_desc_idx
       ATTACH PARTITION p_i_partition_key_id_desc_idx;
   ```

6. NOT NULL proof: same CHECK-NOT-VALID → VALIDATE → SET NOT NULL pattern as
   §3.5, applied at the parent level (propagates to partitions).

7. **Partitioned-parent PK promotion.** Postgres does not support
   `... PRIMARY KEY USING INDEX idx` on a partitioned parent. Use
   `ADD PRIMARY KEY (partition_key, id_desc)` — do not assume catalog-only:

   ```sql
   BEGIN;
       -- Drop the old PK on the partitioned parent (propagates to partitions).
       ALTER TABLE parent DROP CONSTRAINT parent_pkey;

       -- Create the replacement PK in the canonical partitioned-table shape.
       -- The existing UNIQUE index from step 5 can reduce risk, but Postgres
       -- may still scan partitions or build replacement indexes here.
       ALTER TABLE parent ADD PRIMARY KEY (partition_key, id_desc);

       -- The remainder of the cutover (alter default, drop old column,
       -- drop trigger, rename) proceeds as in §7.1's single-table cutover,
       -- but applied to the parent and propagating to partitions.
       ALTER TABLE parent ALTER COLUMN id_desc SET DEFAULT heerid_next_desc();
       ALTER TABLE parent ALTER COLUMN id DROP DEFAULT;
       ALTER TABLE parent DROP COLUMN id;
       DROP TRIGGER zzz_parent_autofill_desc ON parent;
       DROP FUNCTION zzz_parent_autofill_desc() CASCADE;
       ALTER TABLE parent RENAME COLUMN id_desc TO id;
   COMMIT;
   ```

### 9.2 PG 11–12 fallback

BEFORE row triggers on partitioned parents are not auto-routed in these
versions. The install helper must attach a per-partition trigger to each
leaf individually; drop-time iterates per leaf. **Upgrade to PG 13+ before
migrating if at all possible.**

### 9.3 PK-shape requirement

Every parent-level `UNIQUE` or `PRIMARY KEY` must include all partition-key
columns. A bare `(id_desc)` parent key only works when `id_desc` is the
partition key (e.g. `PARTITION BY HASH (id_desc)`), which is rare.

---

## 10. Rollback boundaries (spec §7.6)

- **Before the cutover's `BEGIN;`.** Rollback is free — drop the `_desc`
  columns and triggers; starting state restored; no data loss.
- **Inside the cutover transaction.** `ROLLBACK` or a raised exception
  atomically unwinds every statement; pre-cutover schema restored. Retry
  without manual cleanup.
- **After the cutover commits — the point of no return.** Rollback requires
  a reverse migration: add the asc column back, install a reverse trigger
  (`heerid_to_asc` under the same `zzz_` naming), re-backfill, and run
  another cutover. Plan this contingency before cutting over.

---

## 11. Timing (spec §7.8)

Commodity hardware (local NVMe, no replication lag):

| Phase                                            | Non-partitioned        | Partitioned                                         |
|--------------------------------------------------|------------------------|-----------------------------------------------------|
| Backfill 1M rows at 10k batch size               | Tens of s to a few m   | Same per partition; multiply by count               |
| `CREATE INDEX CONCURRENTLY` on 1M-row bigint     | Tens of s              | Per partition; controlled parallel                  |
| **Cutover transaction**                          | **Milliseconds**       | **Seconds to tens of minutes — BENCHMARK FIRST**    |
| Single-table end-to-end                          | A few minutes          | Depends on partition count / row counts             |
| 10M-row cascade across 5 tables                  | Hours (maintenance window); mostly backfill + index builds | Plus partition multiplication     |

The cutover row is load-bearing: non-partitioned is catalog only; partitioned
is not. Do not extrapolate between them.

---

## 12. Hazards and mitigations (spec §7.9)

### 12.1 `session_replication_role = 'replica'`

**Hazard.** Sessions in `replica` mode suppress triggers whose enable mode is
the default `ORIGIN`, including the autofill. Writes during the window
leave `id_desc` NULL. Typically set by logical-replication apply workers
and by `pg_restore`.

**Mitigation.** Pause such sessions for the window, or mark the trigger
`ALWAYS` so it fires under `replica`:

```sql
ALTER TABLE tbl ENABLE ALWAYS TRIGGER zzz_tbl_autofill_desc;
```

Revert or drop as part of the cutover transaction.

### 12.2 `DISABLE TRIGGER`

**Hazard.** `ALTER TABLE tbl DISABLE TRIGGER USER` (or `ALL`, or the
autofill by name) stops firing regardless of session settings. Any writes
during a disabled window leave `id_desc` unpopulated.

**Mitigation.** Instruct operators not to issue `DISABLE TRIGGER` on the
migrating table during the window; audit for already-disabled triggers
pre-flight (§2). Coordinate with any operational procedure that routinely
disables triggers to pause it for the window.

### 12.3 Logical replication apply workers

**Hazard.** Apply workers default to not firing user triggers; the
subscriber's writes therefore leave `id_desc` NULL.

**Mitigation.** Either (a) pause the subscription, migrate on the publisher,
let replication catch up, then migrate on the subscriber; or (b) mark the
trigger `ALWAYS` (§12.1) so it fires on the subscriber during the window.
(a) is strictly safer; (b) is simpler when the subscriber must stay online.

### 12.4 `TRUNCATE`

**Hazard.** `TRUNCATE` does not fire row triggers by default. Any
truncate-then-insert during the window leaves `id_desc` NULL on reinserted
rows.

**Mitigation.** Defer any `TRUNCATE` on the migrating table until after
migration. If unavoidable during the window, rerun `heeranjid_bulk_backfill`
plus the §3.3 verification before continuing.

### 12.5 Mixed-version application fleets

**Hazard.** Instances still emitting client-generated ascending IDs in
`INSERT ... VALUES (..., $client_id, ...)` bypass the trigger's INSERT
guard (it only populates when `NEW.id_desc IS NULL`). Post-cutover, such
a row has a desc-shape column containing bits that were actually generated
by asc logic — bit-pattern matches, direction doesn't. Likewise, old
instances issuing `ORDER BY id ASC` post-cutover see reverse-chronological
rows.

**Mitigation.** Before cutover, either upgrade every writer to emit
desc-shape IDs (or rely on the server default `heerid_next_desc()`) or
change writers to rely on the default for this table during the window.
Roll old readers forward promptly post-cutover and monitor for query-
result-pattern anomalies.

### 12.6 Read-replica lag during the window

**Hazard.** If the app reads from a replica during the cutover, the replica
may still present the pre-cutover schema when the primary has committed.
New-reader code using `HeerIdDesc` reading pre-cutover asc bits silently
misinterprets them as desc — same bit-pattern, wrong direction.

**Mitigation.** Pick one: (a) pause replica reads during the cutover
window; (b) stall the rollout of the new-reader code until replication
confirms each replica has replayed past the cutover LSN; or (c) route all
reads to the primary during the cutover and for a short buffer after.
Monitor `pg_stat_replication.write_lag` / `flush_lag` / `replay_lag` before
initiating the cutover.

### 12.7 Long-running transactions spanning the cutover

**Hazard.** A transaction that began before the cutover and remains open
will either block the cutover's `AccessExclusiveLock` indefinitely or force
it to abort on `lock_timeout`. The same hazard applies to the cleanup pass
of `heeranjid_bulk_backfill` against a long-held row lock.

**Mitigation.** Use the §2 enumeration query; `pg_cancel_backend(pid)` or
`pg_terminate_backend(pid)` offenders before the final cutover attempt.
Set `SET LOCAL lock_timeout = '5s'` on the cutover session so a missed
long-runner aborts the cutover quickly. Retry after the blocker is cleared.

### 12.8 Informational: physical replicas and `COPY FROM`

Physical streaming replicas replay WAL directly and do not run triggers —
this is bit-for-bit consistency, not a bypass (§12.6 handles the lag
consequences). `COPY FROM` **does** fire row triggers by default — it is
not a bypass unless combined with `session_replication_role = 'replica'` or
a disabled trigger.

### 12.9 Explicit writes to the destination column

**Hazard.** If an app writes directly to `NEW.id_desc`, the trigger's `IF
NEW.id_desc IS NULL` branch leaves that value untouched. This is the
intended behaviour for apps already upgraded to emit desc-shape values, but
is not a "force this value" guarantee.

**Mitigation.** If the fleet contains paths writing to `id_desc` directly,
confirm they emit correct desc-shape bits before cutover.

---

## 13. Monitoring

Run throughout the window, not only during the cutover.

- **Dead-tuple pressure** — the backfill UPDATEs produce dead tuples;
  autovacuum usually keeps up:

  ```sql
  SELECT relname, n_live_tup, n_dead_tup,
         round(100.0 * n_dead_tup / NULLIF(n_live_tup, 0), 2) AS dead_pct
  FROM pg_stat_user_tables
  WHERE relname = 'tbl';
  ```

  If `dead_pct` trends above ~20%, tune autovacuum for the table or run
  manual `VACUUM` between backfill and cutover.

- **Replication lag** on every replica (§12.6):

  ```sql
  SELECT application_name, state, write_lag, flush_lag, replay_lag
  FROM pg_stat_replication;
  ```

- **Lock contention during the cutover** — from a separate monitoring
  session:

  ```sql
  SELECT a.pid, a.query, l.mode, l.granted, now() - a.xact_start AS age
  FROM pg_locks l
  JOIN pg_stat_activity a ON a.pid = l.pid
  WHERE l.relation = 'tbl'::regclass
  ORDER BY a.xact_start NULLS LAST;
  ```

- **Per-write latency on the primary during the double-write window.**
  Expect a small increase (one XOR per relevant column per row). A
  step-change larger than a few hundred microseconds in p99
  insert/update latency indicates something else (lock waits, autovacuum
  contention, index build overhead).

- **Backfill progress.** The procedure emits `RAISE NOTICE 'backfill: %
  rows this batch ...'` per batch; capture the session's notices to a log.

---

## 14. Appendix: SQL reference

### 14.1 Generators

```sql
-- Calls heerid_next() and flips the result.
CREATE OR REPLACE FUNCTION heerid_next_desc() RETURNS bigint AS $$
    SELECT heerid_to_desc(heerid_next());
$$ LANGUAGE sql VOLATILE;

CREATE OR REPLACE FUNCTION ranjid_next_desc() RETURNS uuid AS $$
    SELECT ranjid_to_desc(ranjid_next());
$$ LANGUAGE sql VOLATILE;

-- Bulk counterparts (v0.3.4). Compose the matching asc allocator with
-- the desc flip so callers get a column of descending-shape IDs in a
-- single round-trip — use these from `bulk_create` / range-leasing
-- paths that would otherwise call `heerid_next_desc()` per row.
CREATE OR REPLACE FUNCTION generate_ids_desc(requested_count INTEGER)
RETURNS TABLE(id BIGINT) AS $$
    SELECT heerid_to_desc(id)
    FROM generate_ids(current_heer_node_id(), requested_count, true);
$$ LANGUAGE sql;

CREATE OR REPLACE FUNCTION generate_ranjids_desc(requested_count INTEGER)
RETURNS TABLE(id UUID) AS $$
    SELECT ranjid_to_desc(id)
    FROM generate_ranjids(current_heer_ranj_node_id(), requested_count, true);
$$ LANGUAGE sql;
-- Explicit-node overloads (`(node, count, spanning)`) and the two-arg
-- `(count, spanning)` form exist too; see heeranjid/sql/functions/desc_generators.sql.
```

### 14.2 Flip primitives

```sql
-- The mask constant is exposed as a function so the literal only appears
-- in one place and can be referenced from heerid_to_desc / heerid_to_asc.
-- Written as a decimal literal because Postgres does not accept the
-- `x'...'::bigint` shorthand for signed 64-bit values portably across
-- 13–17; the comment keeps the hex origin explicit.
CREATE OR REPLACE FUNCTION heerid_flip_mask() RETURNS bigint AS $$
    SELECT 9223372036850589695::bigint;  -- 0x7FFFFFFFFFC01FFF
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION heerid_to_desc(bits bigint) RETURNS bigint AS $$
    -- Flip the 41 timestamp bits and 13 sequence bits; leave the 9 node bits alone.
    SELECT (bits # heerid_flip_mask())::bigint;
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION heerid_to_asc(bits bigint) RETURNS bigint AS $$
    -- Symmetric; XOR with the same mask reverses.
    SELECT (bits # heerid_flip_mask())::bigint;
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ranjid_to_desc(id uuid) RETURNS uuid AS $$
DECLARE
    b    bytea := uuid_send(id);
    mask bytea := decode('FFFFFFFFFFFF0FFF0FFFFFFF8000FFFF', 'hex');
    r    bytea := '';
    i    int;
BEGIN
    -- Byte-wise XOR of the 16-byte network-order representation with the
    -- fixed mask. The mask flips ts_high, ts_mid, ts_low, and sequence;
    -- preserves version (4), variant (2), precision (2), node (15).
    FOR i IN 0..15 LOOP
        r := r || set_byte('\x00'::bytea, 0, get_byte(b, i) # get_byte(mask, i));
    END LOOP;
    RETURN encode(r, 'hex')::uuid;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ranjid_to_asc(id uuid) RETURNS uuid AS $$
    -- XOR is symmetric; identical body to ranjid_to_desc.
    SELECT ranjid_to_desc(id);
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;
```

### 14.3 Bulk backfill

Signature: `heeranjid_bulk_backfill(table_name text, src_col text, dst_col
text, kind text, batch_size int DEFAULT 10000)`.

Must be invoked at the top level via `CALL` from a session not already in a
transaction; internal `COMMIT`s otherwise raise `invalid transaction
termination`. Rust wrappers must pass a bare `&PgPool` / `&PgConnection`, not
`&mut PgTransaction<'_>`.

```sql
CREATE OR REPLACE PROCEDURE heeranjid_bulk_backfill(
    table_name text,
    src_col    text,
    dst_col    text,
    kind       text,                       -- 'heer' or 'ranj'
    batch_size int DEFAULT 10000
) LANGUAGE plpgsql AS $$
DECLARE
    flip_fn   text;
    rows_done int;
BEGIN
    flip_fn := CASE kind
        WHEN 'heer' THEN 'heerid_to_desc'
        WHEN 'ranj' THEN 'ranjid_to_desc'
        ELSE NULL
    END;
    IF flip_fn IS NULL THEN
        RAISE EXCEPTION 'heeranjid_bulk_backfill: unknown kind %', kind;
    END IF;

    LOOP
        -- SET LOCAL must be reissued after each COMMIT because its scope is
        -- the current transaction (PG docs: plpgsql-transactions). Issuing
        -- it at the top of the loop body means every batch transaction runs
        -- with the 30s lock_timeout.
        SET LOCAL lock_timeout = '30s';
        EXECUTE format(
            'WITH batch AS (
                 SELECT ctid FROM %I
                 WHERE %I IS NULL AND %I IS NOT NULL
                 LIMIT %s
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE %I t SET %I = %I(t.%I)
             FROM batch
             WHERE t.ctid = batch.ctid',
            table_name, dst_col, src_col, batch_size,
            table_name, dst_col, flip_fn, src_col
        );
        GET DIAGNOSTICS rows_done = ROW_COUNT;
        COMMIT;
        RAISE NOTICE 'backfill: % rows this batch (skip-locked)', rows_done;
        EXIT WHEN rows_done = 0;
    END LOOP;

    -- Cleanup pass: catch rows that were locked by long-running transactions
    -- every time the loop saw them (SKIP LOCKED hides them indefinitely). This
    -- pass does NOT use SKIP LOCKED and can block indefinitely on a live
    -- locker unless lock_timeout aborts the statement first; by this point the
    -- residue is usually small. Still filters src IS NOT NULL so nullable FKs
    -- are not touched.
    LOOP
        -- Same rationale as above: SET LOCAL is transaction-scoped and must
        -- be reissued per batch.
        SET LOCAL lock_timeout = '30s';
        EXECUTE format(
            'WITH batch AS (
                 SELECT ctid FROM %I
                 WHERE %I IS NULL AND %I IS NOT NULL
                 LIMIT %s
                 FOR UPDATE
             )
             UPDATE %I t SET %I = %I(t.%I)
             FROM batch
             WHERE t.ctid = batch.ctid',
            table_name, dst_col, src_col, batch_size,
            table_name, dst_col, flip_fn, src_col
        );
        GET DIAGNOSTICS rows_done = ROW_COUNT;
        COMMIT;
        RAISE NOTICE 'backfill: % rows this batch (cleanup)', rows_done;
        EXIT WHEN rows_done = 0;
    END LOOP;
END;
$$;
```

### 14.4 `ColumnPair`-style multi-column trigger install

Rust surface:

```rust
pub async fn install_autofill_trigger_for_table<E>(
    exec: &mut E,
    table: &str,
    pairs: &[ColumnPair<'_>],           // at least one; multiple for self-FK / join tables
    kind: IdKind,                       // IdKind::Heer or IdKind::Ranj
) -> Result<(), sqlx::Error>;

pub async fn drop_autofill_trigger_for_table<E>(
    exec: &mut E,
    table: &str,
) -> Result<(), sqlx::Error>;
```

Single-pair generated trigger (worked example — table `events`, pair `id →
id_desc`, kind Heer):

```sql
CREATE OR REPLACE FUNCTION zzz_events_autofill_desc() RETURNS trigger AS $$
BEGIN
    -- One branch per (src, dst) pair. Below is the one-pair shape; see
    -- §7.4 and §7.5 for the multi-pair cases (self-FK and join tables).
    IF TG_OP = 'INSERT' THEN
        IF NEW.id_desc IS NULL THEN
            NEW.id_desc := heerid_to_desc(NEW.id);
        END IF;
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.id IS DISTINCT FROM OLD.id THEN
            NEW.id_desc := heerid_to_desc(NEW.id);
        ELSIF NEW.id_desc IS NULL THEN
            NEW.id_desc := heerid_to_desc(NEW.id);
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger name is prefixed with `zzz_` so it sorts LAST alphabetically.
-- Postgres fires BEFORE triggers in name order (pg_trigger.tgname ascending);
-- other BEFORE triggers on the same table would otherwise run after ours and
-- could mutate NEW.id, leaving NEW.id_desc stale within the same statement.
-- Running last makes NEW.id's final value the one we flip.
CREATE TRIGGER zzz_events_autofill_desc
    BEFORE INSERT OR UPDATE ON events
    FOR EACH ROW EXECUTE FUNCTION zzz_events_autofill_desc();
```

Multi-pair generated trigger (worked self-FK example — table `nodes`, pairs
`id → id_desc` and `parent_id → parent_id_desc`, kind Heer):

```sql
CREATE OR REPLACE FUNCTION zzz_nodes_autofill_desc() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.id_desc        IS NULL THEN NEW.id_desc        := heerid_to_desc(NEW.id);        END IF;
        IF NEW.parent_id_desc IS NULL THEN NEW.parent_id_desc := heerid_to_desc(NEW.parent_id); END IF;
    ELSIF TG_OP = 'UPDATE' THEN
        IF    NEW.id        IS DISTINCT FROM OLD.id        THEN NEW.id_desc        := heerid_to_desc(NEW.id);
        ELSIF NEW.id_desc   IS NULL                        THEN NEW.id_desc        := heerid_to_desc(NEW.id);
        END IF;
        IF    NEW.parent_id IS DISTINCT FROM OLD.parent_id THEN NEW.parent_id_desc := heerid_to_desc(NEW.parent_id);
        ELSIF NEW.parent_id_desc IS NULL                   THEN NEW.parent_id_desc := heerid_to_desc(NEW.parent_id);
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

The UPDATE branch handles the migration race: if the source PK or FK
changes between the backfill pass and the cutover, the stale desc is
recomputed automatically. For join tables (§7), substitute the two FK pairs
(e.g. `p1_id → p1_id_desc`, `p2_id → p2_id_desc`). For RanjId tables,
substitute `ranjid_to_desc` for `heerid_to_desc` throughout.
