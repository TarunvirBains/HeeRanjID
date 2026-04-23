# Migration playbook: ascending to descending sort IDs (MSSQL)

Operator-facing runbook for migrating a live table on **MSSQL** from
ascending `HeerId` / `RanjId` to their descending siblings without
downtime. The companion doc `asc-to-desc.md` covers the Postgres path;
design rationale lives in
`docs/superpowers/specs/2026-04-22-descending-sort-ids-design.md` and
the v0.3.1 spec
`docs/superpowers/specs/2026-04-23-v0.3.1-mssql-and-django-desc-design.md`.

SQL reproduced here is the canonical T-SQL shipped in
`heeranjid/sql/mssql/procedures/*.sql`. Change only identifiers when
adapting; do not paraphrase procedure bodies.

---

## 1. When to migrate

Migrate when one or more applies:

- **Chronological queries dominate.** Most reads are `ORDER BY id DESC
  OFFSET N ROWS FETCH NEXT M ROWS ONLY` or reverse keyset pagination.
  The descending variant turns those into a natural forward scan of
  the clustered index.
- **Dedicated DESC clustered or covering index exists only to accelerate
  those reads.** Dropping it reclaims storage, shrinks the log volume
  on each write, and removes index-maintenance work.
- **Read-path simplification.** Removing explicit `ORDER BY ... DESC`
  everywhere (especially in ORM-generated T-SQL) reduces query surface
  area.

If none apply, stay on the ascending variant. MSSQL's descending-order
index hint is cheap; the migration itself is not.

---

## 2. Pre-flight audit (MSSQL-specific)

Run every item before touching the table.

### 2.1 Node id assigned

The migrating service must have bound its node id via
`heer_set_node_id` / `heer_set_ranj_node_id`:

```sql
SELECT dbo.heer_current_node_id();       -- must return a value in range
SELECT dbo.heer_current_ranj_node_id();  -- if using RanjId
```

### 2.2 Generators reachable

```sql
EXEC dbo.generate_id @in_node_id = NULL;      -- must succeed (returns one row)
EXEC dbo.generate_ranjid @in_node_id = NULL;  -- if using RanjId
EXEC dbo.heerid_next_desc;                    -- v0.3.1 wrapper
EXEC dbo.ranjid_next_desc;                    -- v0.3.1 wrapper
```

### 2.3 No `zzz_*` trigger already on the target table

The autofill is named `zzz_<table>_autofill_desc`. A collision aborts
install; any other `AFTER INSERT, UPDATE` trigger with
`ExecIsLastInsertTrigger = 1` would conflict with our `sp_settriggerorder
@order = 'Last'` claim.

```sql
SELECT
    t.name AS trigger_name,
    OBJECTPROPERTY(t.object_id, 'ExecIsLastInsertTrigger') AS is_last_insert,
    OBJECTPROPERTY(t.object_id, 'ExecIsLastUpdateTrigger') AS is_last_update
FROM sys.triggers t
WHERE t.parent_id = OBJECT_ID('dbo.<table>')
ORDER BY t.name;
```

Any row with `is_last_insert = 1` or `is_last_update = 1` competes for
the `sp_settriggerorder 'Last'` slot. Resolve before proceeding —
either drop the competing trigger or hand-wire a custom trigger that
composes both behaviors. See §12.1 for the composition pattern.

### 2.4 Edition check (Enterprise vs Standard)

`CREATE INDEX ... WITH (ONLINE = ON)` is Enterprise-edition-only on
tables larger than a few million rows. Standard / Developer / Web
editions require `ONLINE = OFF`, which takes a table-level lock for
the duration of the index build.

```sql
SELECT SERVERPROPERTY('Edition') AS edition,
       SERVERPROPERTY('EngineEdition') AS engine_edition;
-- EngineEdition: 2 = Standard, 3 = Enterprise, 4 = Express, 8 = Managed Instance
```

If not Enterprise, schedule a maintenance window for the index build
in §3, step 5. The rest of the playbook runs unchanged.

### 2.5 Always On AG / replication audit

Asc↔desc migration on a primary should flow cleanly through Always On
AG and transactional replication — both preserve trigger side effects.
But confirm:

```sql
-- Listener / sync commit state
SELECT ag.name, rs.is_local, rs.synchronization_state_desc
FROM sys.dm_hadr_availability_replica_states rs
JOIN sys.availability_groups ag ON ag.group_id = rs.group_id;

-- Publications that include the target table
SELECT pub.name, art.name
FROM distribution.dbo.MSpublications pub
JOIN distribution.dbo.MSarticles art ON art.publication_id = pub.publication_id
WHERE art.source_object = OBJECT_NAME(OBJECT_ID('<table>'));
```

If the table is in a merge replication publication, **stop** — merge
replication rewrites triggers and will silently disable the autofill.
Convert to transactional replication first, or accept that the
migration runs on the publisher only and consumers see stale `id_desc`
until a full resync.

### 2.6 Long-running transactions identified

```sql
SELECT
    r.session_id,
    r.start_time,
    DATEDIFF(second, r.start_time, GETUTCDATE()) AS age_seconds,
    r.status,
    r.command,
    r.wait_type,
    t.text AS sql_text
FROM sys.dm_exec_requests r
CROSS APPLY sys.dm_exec_sql_text(r.sql_handle) t
WHERE DATEDIFF(second, r.start_time, GETUTCDATE()) > 60
ORDER BY r.start_time;
```

Any session over 60 s old will either block the cutover's schema lock
or force `LOCK_TIMEOUT` to abort it. Either pause them or snooze the
cutover until they clear.

### 2.7 MSSQL version

Minimum: SQL Server **2017** for `CREATE OR ALTER TRIGGER` and `THROW`.
The v0.2.x MSSQL schema ships on 2022; v0.3.1 tightens the documented
minimum to 2017. Query:

```sql
SELECT SERVERPROPERTY('ProductVersion'), SERVERPROPERTY('ProductLevel');
```

---

## 3. Single-table recipe

Five transaction contexts: preparation (each statement auto-commits),
trigger install (auto-commit), backfill (procedure manages its own
transactions via `BEGIN TRAN / COMMIT TRAN` per batch), index build
(auto-commit, `ONLINE = ON` if Enterprise), and cutover (one
`BEGIN TRAN / COMMIT TRAN`).

### 3.1 Add the `id_desc` column (offline, fast)

```sql
ALTER TABLE dbo.<table> ADD id_desc bigint NULL;  -- HeerId
-- or for RanjId:
ALTER TABLE dbo.<table> ADD id_desc BINARY(16) NULL;
```

MSSQL adds the NULL column as a metadata-only operation — no row
rewrite. Instant regardless of table size.

### 3.2 Install the autofill trigger

Use `heeranjid.mssql_schema.install_autofill_trigger_for_table` (Python)
or call `heeranjid::mssql_schema::install_autofill_trigger_for_table_mssql`
(Rust) to generate the T-SQL, then execute it:

```sql
CREATE OR ALTER TRIGGER zzz_<table>_autofill_desc
ON dbo.<table>
AFTER INSERT, UPDATE
AS
BEGIN
    SET NOCOUNT ON;
    UPDATE t
    SET t.id_desc = CASE WHEN t.id IS NULL THEN NULL ELSE dbo.heerid_to_desc(t.id) END
    FROM dbo.<table> AS t
    WHERE t.id IN (SELECT id FROM inserted WHERE id IS NOT NULL);
END;
GO
EXEC sp_settriggerorder @triggername = N'zzz_<table>_autofill_desc', @order = 'Last', @stmttype = 'INSERT';
GO
EXEC sp_settriggerorder @triggername = N'zzz_<table>_autofill_desc', @order = 'Last', @stmttype = 'UPDATE';
GO
```

### 3.3 Backfill existing rows

```sql
EXEC dbo.heeranjid_bulk_backfill
    @table_name = N'<table>',
    @src_col    = N'id',
    @dst_col    = N'id_desc',
    @kind       = 'heer',           -- or 'ranj'
    @batch_size = 10000;
```

The procedure loops in 10 000-row batches, each in its own `BEGIN TRAN
/ COMMIT TRAN`. `SET LOCK_TIMEOUT 30000` (30 s) is session-scoped and
set once at proc entry. Fast loop uses `UPDLOCK, READPAST` (MSSQL's
`SKIP LOCKED`); cleanup loop uses plain `UPDLOCK`.

Expected runtime: ~1–2 minutes per 10 M rows on NVMe. Monitor via §10.

### 3.4 Index build

Enterprise edition (online):

```sql
CREATE UNIQUE NONCLUSTERED INDEX ix_<table>_id_desc
ON dbo.<table> (id_desc)
WITH (ONLINE = ON, MAXDOP = 4);
```

Standard edition (blocking — requires maintenance window):

```sql
CREATE UNIQUE NONCLUSTERED INDEX ix_<table>_id_desc
ON dbo.<table> (id_desc)
WITH (ONLINE = OFF);
```

The index is initially non-clustered. We'll promote it to the PK in
§3.7.

### 3.5 NOT NULL upgrade

After backfill completes and the index is built, tighten the column:

```sql
ALTER TABLE dbo.<table> ALTER COLUMN id_desc bigint NOT NULL;
-- or for RanjId:
ALTER TABLE dbo.<table> ALTER COLUMN id_desc BINARY(16) NOT NULL;
```

MSSQL rewrites the column to remove the nullability bit. Relatively
fast (metadata-only on 2019+ when the column has no NULLs; full
rewrite on older versions).

### 3.6 Verify no stragglers

```sql
SELECT COUNT(*) FROM dbo.<table> WHERE id_desc IS NULL AND id IS NOT NULL;
-- Must be zero before cutover.
```

### 3.7 Cutover transaction

One transaction flips the PK. Swapping the clustered index is the
heavy step — consider scheduling during a quiet minute.

```sql
BEGIN TRAN;

    -- 1. Drop the old PK (releases clustered index on id)
    ALTER TABLE dbo.<table> DROP CONSTRAINT PK_<table>;

    -- 2. Drop the old ascending index (now redundant with id_desc)
    DROP INDEX ix_<table>_id_desc ON dbo.<table>;

    -- 3. Create the new PK on id_desc (clustered)
    ALTER TABLE dbo.<table>
        ADD CONSTRAINT PK_<table> PRIMARY KEY CLUSTERED (id_desc);

    -- 4. Add a non-clustered index on id (keeps any FK references
    --    still using `id` as the lookup column efficient)
    CREATE NONCLUSTERED INDEX ix_<table>_id ON dbo.<table> (id);

COMMIT TRAN;
```

For larger tables, split: drop the old PK in a first transaction,
build the clustered index on `id_desc` outside any transaction (so
`ONLINE = ON` works), then add the PK constraint using the existing
index:

```sql
-- Outside transaction
CREATE UNIQUE CLUSTERED INDEX PK_<table>_idx ON dbo.<table> (id_desc)
    WITH (ONLINE = ON, DROP_EXISTING = OFF);

BEGIN TRAN;
    ALTER TABLE dbo.<table> DROP CONSTRAINT PK_<table>;
    ALTER TABLE dbo.<table>
        ADD CONSTRAINT PK_<table> PRIMARY KEY CLUSTERED (id_desc)
        WITH (DROP_EXISTING = OFF);
    -- ...
COMMIT TRAN;
```

### 3.8 Post-cutover validation

```sql
-- Row count preserved
SELECT COUNT(*) FROM dbo.<table>;

-- Descending order works off the PK
SELECT TOP 100 * FROM dbo.<table> ORDER BY id_desc;

-- Old column is still there (phase out later; keep for rollback
-- window — see §6).
SELECT TOP 5 id, id_desc FROM dbo.<table>;
```

### 3.9 Drop the old `id` column (deferred)

After a rollback window (48–72 hours of observed stability is typical),
drop the ascending column. This is a separate change — it breaks the
rollback boundary (§6).

```sql
-- Drop any indexes / constraints referencing the old column first
DROP INDEX ix_<table>_id ON dbo.<table>;

-- Finally
ALTER TABLE dbo.<table> DROP COLUMN id;
```

If any code still reads `id`, it will throw — pre-audit by searching
your codebase for literal references to the old column.

---

## 4. Parent + child FK

When the migrating table has FKs referencing it, each FK column also
needs an `<fk>_desc` sibling. MSSQL has **no deferred constraints**
(unlike Postgres's `DEFERRABLE INITIALLY DEFERRED`), so the cutover
needs drop-and-recreate instead.

### 4.1 Parent-first approach

1. On the parent: §3.1–§3.6 (add column, install trigger, backfill,
   index, NOT NULL, verify).
2. On **each child table** independently: §3.1–§3.6 for the FK column.
   The trigger emits `UPDATE child SET fk_desc = flip(fk) WHERE fk IN
   (SELECT fk FROM inserted)`.
3. Cutover **both parent and children inside a single transaction**
   (since the FK from child to parent must reference `id_desc` from
   the moment `id` stops being the PK):

```sql
BEGIN TRAN;

    -- Parent: swap PK (see §3.7)
    ALTER TABLE dbo.<parent> DROP CONSTRAINT PK_<parent>;
    ALTER TABLE dbo.<parent> ADD CONSTRAINT PK_<parent>
        PRIMARY KEY CLUSTERED (id_desc);

    -- Children: drop old FKs, drop old fk columns, rename fk_desc
    -- columns to fk, add new FK referencing parent.id_desc
    ALTER TABLE dbo.<child> DROP CONSTRAINT FK_<child>_<parent>;
    ALTER TABLE dbo.<child> DROP COLUMN fk;
    EXEC sp_rename 'dbo.<child>.fk_desc', 'fk', 'COLUMN';
    ALTER TABLE dbo.<child>
        ADD CONSTRAINT FK_<child>_<parent> FOREIGN KEY (fk)
        REFERENCES dbo.<parent> (id_desc);

COMMIT TRAN;
```

### 4.2 Cascade and ON DELETE/UPDATE behavior

MSSQL allows at most one `CASCADE` path through a given table — check
`ON DELETE CASCADE` and `ON UPDATE CASCADE` on the original FKs:

```sql
SELECT
    fk.name AS fk_name,
    fk.delete_referential_action_desc AS on_delete,
    fk.update_referential_action_desc AS on_update
FROM sys.foreign_keys fk
WHERE fk.referenced_object_id = OBJECT_ID('dbo.<parent>');
```

Reproduce the same `ON DELETE` / `ON UPDATE` clauses in the recreated
constraint, or the cascade behavior silently regresses.

---

## 5. Multi-level cascade

Recurse §4. For each level:

1. Install the autofill trigger (§3.2).
2. Backfill (§3.3).
3. Verify (§3.6).

All tables in the dependency chain must complete §3.1–§3.6 before the
collective cutover. The cutover itself is one transaction that swaps
all PKs and FKs top-down.

Memory hint: `sys.foreign_keys` ordered by reference depth:

```sql
WITH dep AS (
    SELECT
        OBJECT_NAME(fk.parent_object_id) AS child,
        OBJECT_NAME(fk.referenced_object_id) AS parent,
        0 AS depth
    FROM sys.foreign_keys fk
    WHERE OBJECT_NAME(fk.referenced_object_id) = '<root>'
    UNION ALL
    SELECT
        OBJECT_NAME(fk.parent_object_id),
        OBJECT_NAME(fk.referenced_object_id),
        dep.depth + 1
    FROM sys.foreign_keys fk
    JOIN dep ON dep.child = OBJECT_NAME(fk.referenced_object_id)
)
SELECT DISTINCT child, parent, depth FROM dep ORDER BY depth, parent;
```

---

## 6. Self-FK (self-referential tables)

A table whose FK points back at its own PK (e.g., `nodes(id, parent_id)`)
is the classic hard case. Postgres solves this with deferrable
constraints so the whole swap happens atomically; MSSQL has no such
mechanism.

### 6.1 MSSQL approach: drop-and-recreate inside cutover

1. Pre-flight (§3.1–§3.6): add `id_desc` AND `parent_id_desc`, install
   a multi-pair trigger, backfill both columns, index both.

   The multi-pair trigger call:

   ```python
   from heeranjid import mssql_schema
   sql = mssql_schema.install_autofill_trigger_for_table(
       table="nodes",
       pairs=[("id", "id_desc"), ("parent_id", "parent_id_desc")],
       kind="heer",
   )
   ```

2. Cutover:

```sql
BEGIN TRAN;

    -- Drop self-FK
    ALTER TABLE dbo.nodes DROP CONSTRAINT FK_nodes_parent_id;

    -- Swap PK (see §3.7)
    ALTER TABLE dbo.nodes DROP CONSTRAINT PK_nodes;
    ALTER TABLE dbo.nodes ADD CONSTRAINT PK_nodes
        PRIMARY KEY CLUSTERED (id_desc);

    -- Drop old parent_id, rename parent_id_desc -> parent_id
    ALTER TABLE dbo.nodes DROP COLUMN parent_id;
    EXEC sp_rename 'dbo.nodes.parent_id_desc', 'parent_id', 'COLUMN';

    -- Recreate self-FK pointing at id_desc
    ALTER TABLE dbo.nodes ADD CONSTRAINT FK_nodes_parent_id
        FOREIGN KEY (parent_id) REFERENCES dbo.nodes (id_desc);

COMMIT TRAN;
```

Both `id` and `parent_id` stay atomically consistent through the swap
because everything happens inside one transaction. The brief FK-less
window is invisible to concurrent transactions (they see the pre-image).

### 6.2 `WITH CHECK` on FK recreate

By default, `ALTER TABLE ... ADD CONSTRAINT FOREIGN KEY` is `WITH
CHECK` — the DB validates every row. On a large self-referential
table, skip the check if you've already validated via backfill:

```sql
ALTER TABLE dbo.nodes WITH NOCHECK ADD CONSTRAINT FK_nodes_parent_id
    FOREIGN KEY (parent_id) REFERENCES dbo.nodes (id_desc);
-- Then, outside the cutover transaction:
ALTER TABLE dbo.nodes WITH CHECK CHECK CONSTRAINT FK_nodes_parent_id;
```

The second statement runs the check with the constraint trusted,
without holding the cutover transaction longer. Do this only if you
have high confidence in the backfill (verified via §3.6 and §3.8).

---

## 7. Join tables

A join table with two FKs referencing different parents is §4 applied
twice independently:

1. Migrate parent A (full §3.1–§3.9 cycle minus §3.9).
2. Migrate parent B (full cycle minus §3.9).
3. Migrate the join table: add both `fk_a_desc` and `fk_b_desc`,
   install a multi-pair trigger, backfill, index, cutover all three
   (parent A, parent B, join) in **one transaction**.

If the join table has additional FK semantics (soft-delete flag,
composite PK including the FKs), the multi-pair trigger pattern from
§6 extends naturally.

---

## 8. Cycles

Cycles in the FK graph (A → B → C → A) **force** drop-and-recreate of
at least one FK inside the cutover — no deferred-constraint escape
hatch. Break the cycle at the weakest edge (usually the one with the
lowest referenced-row churn):

```sql
BEGIN TRAN;

    -- Break the cycle: drop C → A
    ALTER TABLE dbo.C DROP CONSTRAINT FK_C_A;

    -- Now A, B, C form a DAG; migrate in topological order per §4
    ALTER TABLE dbo.A DROP CONSTRAINT PK_A;
    ALTER TABLE dbo.A ADD CONSTRAINT PK_A PRIMARY KEY CLUSTERED (id_desc);
    -- ... (swap A's column, then B's FK, then B's PK, then C's FK to B)

    -- Restore the broken edge
    ALTER TABLE dbo.C ADD CONSTRAINT FK_C_A
        FOREIGN KEY (a_id) REFERENCES dbo.A (id_desc);

COMMIT TRAN;
```

If the cycle spans > 3 tables, consider splitting the cutover across
two transactions with a consistent intermediate schema state. The
trade-off is a brief window where the cycle is half-migrated;
read-only workloads can tolerate it, but writes see FK violations if
they touch the open edge.

---

## 9. Partitioned tables

Partitioned tables on MSSQL (partition function + partition scheme)
need special handling:

- **`CREATE INDEX ... WITH (ONLINE = ON)` on a partitioned table**
  requires Enterprise edition and the `ALLOW_PAGE_LOCKS = ON` option.
  Check before attempting.
- **Partition switching** during migration is incompatible with the
  autofill trigger (`SWITCH PARTITION` doesn't fire triggers). Pause
  partition maintenance for the migration window.

**Out of scope for v0.3.1:** automated partitioned-table migration
helpers. The v0.2.x schema doesn't emit partitioned tables, and the
v0.3.1 helpers don't test against them. Operators with partitioned
tables should follow §3 per partition or switch partitions out,
migrate, and switch back.

---

## 10. Rollback boundaries

- **Before §3.7 cutover**: free rollback. Drop the `id_desc` column,
  drop the trigger, drop the index. Zero data loss.
- **After §3.7, before §3.9 (drop `id`)**: rollback means reverse-
  migration — swap PK back to `id`, rebuild any dropped FKs. The
  trigger + backfill keep `id` valid, so data-correctness is
  preserved. Timing penalty matches the forward cutover.
- **After §3.9**: rollback requires re-migrating `id_desc → id` as a
  fresh cycle. Treat as a forward migration in the opposite direction.

Rollback window recommendation: 48–72 hours between §3.7 and §3.9.

---

## 11. Timing

Rough numbers on NVMe, SQL Server 2022 Standard, one non-clustered
index besides the PK. Scale linearly with row count beyond 10 M.

| Table size | §3.1 ADD | §3.3 backfill | §3.4 index (ONLINE) | §3.7 cutover | Total |
|---|---|---|---|---|---|
| 100 K | <100 ms | 2–5 s | 5–10 s | <1 s | ~20 s |
| 10 M | <100 ms | 1–3 min | 1–3 min | 5–15 s | ~5 min |
| 100 M | <100 ms | 10–30 min | 10–30 min | 30–60 s | ~1 h |
| 1 B | <100 ms | 2–6 h | 2–6 h | 3–10 min | ~12 h |

**Write amplification note.** During the autofill-trigger window
(post-§3.2, pre-§3.9), every INSERT and UPDATE fires one additional
UPDATE statement (the AFTER trigger). MSSQL has no BEFORE row
triggers, so we cannot fold the autofill into the base write. For
very high-write hot paths, benchmark the trigger overhead against
your write SLOs before committing to the migration window.

---

## 12. Hazards & mitigations

### 12.1 `BULK INSERT` / `INSERT BULK` with `FIRE_TRIGGERS` unset

By default, `BULK INSERT` and `bcp` **skip triggers** — the autofill
won't populate `id_desc` for bulk-loaded rows. Hazard mitigation:

```sql
BULK INSERT dbo.<table> FROM '...' WITH (FIRE_TRIGGERS);
-- or bcp with the -h "FIRE_TRIGGERS" hint
```

Pre-audit any ETL scripts for bulk-load usage. If trigger-bypassing
loads are unavoidable (performance), follow up with
`heeranjid_bulk_backfill` to flip any rows the bulk load inserted.

### 12.2 `WITH (NO_TRIGGERS)` hint

A query-level `UPDATE dbo.<table> WITH (NO_TRIGGERS) ... ` bypasses
the autofill. Audit for this hint:

```bash
grep -rEi 'WITH[[:space:]]*\([^)]*NO_TRIGGERS' path/to/sources/
```

Mitigation: same as §12.1 — follow the hinted write with a bulk
backfill call, or remove the hint.

### 12.3 Always On AG

Triggers fire on the primary; transaction log entries replay on
secondaries including the AFTER-trigger side effects (this is how
MSSQL replication preserves trigger semantics). No action needed for
AG-replicated tables.

### 12.4 Transactional replication

As with AG, transactional replication preserves trigger side effects
on subscribers — the subscriber executes the same transaction
including the trigger's UPDATE. No action needed.

### 12.5 Merge replication

Merge replication **rewrites** triggers; it will conflict with the
autofill. **Do not migrate a merge-published table.** Migrate the
publisher, stop merge replication, convert to transactional, then
resume.

### 12.6 Long-running transactions spanning cutover

A transaction started before §3.7 and still open during cutover holds
a shared schema lock. The cutover's `ALTER TABLE` needs a schema
modification lock (Sch-M), which is exclusive. `LOCK_TIMEOUT`
(inherited from the session setting) aborts the cutover if the wait
exceeds the threshold. Choose:

- Pause the offending sessions before cutover.
- Set a generous `SET LOCK_TIMEOUT 60000;` (60 s) on the cutover
  session and retry on abort.
- Use `ALTER TABLE ... WAIT_AT_LOW_PRIORITY` (Enterprise-only, SQL
  2014+) to queue behind without blocking unrelated work.

### 12.7 `TRUNCATE TABLE` bypasses triggers

Same as Postgres: `TRUNCATE` is a DDL operation and does not fire
DML triggers. The autofill won't run, but `TRUNCATE` also deletes all
rows so `id_desc` disappears with `id`. Post-truncate reloads via
`INSERT` fire the trigger normally.

### 12.8 `OUTPUT` clause side effects

`INSERT ... OUTPUT inserted.*` returns the post-insert row including
the autofill-populated `id_desc`. ORMs that read `inserted.id` back
via `OUTPUT` get `id_desc` for free — Django's `pre_save` dispatch
doesn't need adjustment.

---

## 13. Monitoring

During backfill and cutover, watch:

```sql
-- Active locks on the migrating table
SELECT
    l.request_session_id,
    l.resource_type,
    l.resource_description,
    l.request_mode,
    l.request_status,
    r.command,
    t.text AS sql_text
FROM sys.dm_tran_locks l
LEFT JOIN sys.dm_exec_requests r ON r.session_id = l.request_session_id
OUTER APPLY sys.dm_exec_sql_text(r.sql_handle) t
WHERE l.resource_associated_entity_id = OBJECT_ID('dbo.<table>')
ORDER BY l.request_session_id;

-- Index physical stats (post-cutover fragmentation)
SELECT
    OBJECT_NAME(ps.object_id) AS table_name,
    i.name AS index_name,
    ps.avg_fragmentation_in_percent,
    ps.page_count
FROM sys.dm_db_index_physical_stats(DB_ID(), OBJECT_ID('dbo.<table>'), NULL, NULL, 'LIMITED') ps
JOIN sys.indexes i ON i.object_id = ps.object_id AND i.index_id = ps.index_id
ORDER BY i.index_id;

-- Trigger execution stats (requires DBCC PROCCACHE)
SELECT
    OBJECT_NAME(qs.object_id) AS trigger_name,
    qs.cached_time,
    qs.execution_count,
    qs.total_worker_time / NULLIF(qs.execution_count, 0) AS avg_cpu_time_us
FROM sys.dm_exec_procedure_stats qs
WHERE OBJECT_NAME(qs.object_id) LIKE 'zzz_%_autofill_desc'
ORDER BY qs.execution_count DESC;
```

---

## 14. Appendix: T-SQL reference

All objects are installed under `dbo.` via
`heeranjid/sql/mssql/procedures/*.sql`.

### 14.1 Flip primitives (desc_flip.sql)

```sql
dbo.heerid_flip_mask() → bigint         -- Returns 0x7FFFFFFFFFC01FFF
dbo.heerid_to_desc(@bits bigint) → bigint
dbo.heerid_to_asc(@bits bigint) → bigint
dbo.ranjid_to_desc(@id BINARY(16)) → BINARY(16)
dbo.ranjid_to_asc(@id BINARY(16)) → BINARY(16)
```

### 14.2 Desc generators (desc_generators.sql)

```sql
EXEC dbo.heerid_next_desc @in_node_id = NULL;    -- Returns one row: id bigint
EXEC dbo.ranjid_next_desc @in_node_id = NULL;    -- Returns one row: id BINARY(16)
```

### 14.3 Bulk backfill (bulk_backfill.sql)

```sql
EXEC dbo.heeranjid_bulk_backfill
    @table_name = N'events',     -- target table
    @src_col    = N'id',         -- source (asc) column
    @dst_col    = N'id_desc',    -- destination (desc) column
    @kind       = 'heer',        -- 'heer' or 'ranj'
    @batch_size = 10000;         -- default 10000
```

### 14.4 Autofill trigger install (single-pair)

```sql
-- Generated by heeranjid::mssql_schema::install_autofill_trigger_for_table_mssql
-- (Rust) or heeranjid.mssql_schema.install_autofill_trigger_for_table (Python):

CREATE OR ALTER TRIGGER zzz_events_autofill_desc
ON dbo.events
AFTER INSERT, UPDATE
AS
BEGIN
    SET NOCOUNT ON;
    UPDATE t
    SET t.id_desc = CASE WHEN t.id IS NULL THEN NULL ELSE dbo.heerid_to_desc(t.id) END
    FROM dbo.events AS t
    WHERE t.id IN (SELECT id FROM inserted WHERE id IS NOT NULL);
END;
GO
EXEC sp_settriggerorder @triggername = N'zzz_events_autofill_desc', @order = 'Last', @stmttype = 'INSERT';
GO
EXEC sp_settriggerorder @triggername = N'zzz_events_autofill_desc', @order = 'Last', @stmttype = 'UPDATE';
GO
```

### 14.5 Autofill trigger install (self-FK, multi-pair)

```sql
CREATE OR ALTER TRIGGER zzz_nodes_autofill_desc
ON dbo.nodes
AFTER INSERT, UPDATE
AS
BEGIN
    SET NOCOUNT ON;
    UPDATE t
    SET t.id_desc = CASE WHEN t.id IS NULL THEN NULL ELSE dbo.heerid_to_desc(t.id) END
    FROM dbo.nodes AS t
    WHERE t.id IN (SELECT id FROM inserted WHERE id IS NOT NULL);
    UPDATE t
    SET t.parent_id_desc = CASE WHEN t.parent_id IS NULL THEN NULL ELSE dbo.heerid_to_desc(t.parent_id) END
    FROM dbo.nodes AS t
    WHERE t.parent_id IN (SELECT parent_id FROM inserted WHERE parent_id IS NOT NULL);
END;
GO
EXEC sp_settriggerorder @triggername = N'zzz_nodes_autofill_desc', @order = 'Last', @stmttype = 'INSERT';
GO
EXEC sp_settriggerorder @triggername = N'zzz_nodes_autofill_desc', @order = 'Last', @stmttype = 'UPDATE';
GO
```

### 14.6 Autofill trigger drop

```sql
IF OBJECT_ID(N'zzz_events_autofill_desc', N'TR') IS NOT NULL
    DROP TRIGGER zzz_events_autofill_desc;
GO
```

---

## Cross-references

- Postgres companion: `docs/migrations/asc-to-desc.md`
- Design spec: `docs/superpowers/specs/2026-04-22-descending-sort-ids-design.md`
- v0.3.1 MSSQL spec: `docs/superpowers/specs/2026-04-23-v0.3.1-mssql-and-django-desc-design.md`
- Django-specific flow: `docs/guide/django-migrations.md`
