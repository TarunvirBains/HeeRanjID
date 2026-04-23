# Benchmark Scripts: ID Generator Throughput Under Contention

This directory contains benchmark scripts to measure HeerId and RanjId generator throughput when
multiple concurrent writers contend on a **single fixed node**. The scripts help validate whether
your database design hits a practical throughput ceiling before considering architectural changes.

## Why Fixed node_id Matters

**Key insight:** Randomizing node_id across multiple nodes makes the test embarrassingly parallel
— each writer operates on a different state row and does not contend. This measures database and
network latency, not the design's hotspot behavior.

By fixing `node_id=42` (or any constant), every concurrent writer locks the same `heer_node_state`
or `heer_ranj_node_state` row. This forces serialization at the database level and is the only
regime where the design could create a throughput bottleneck.

## What the Benchmarks Measure

### 1. Single-ID TPS (Throughput Floor)

- **Script:** `script_single.sql` (pgbench) or `script_single.sql` (ostress)
- **What it does:** Calls `generate_id(42)` once per transaction (single ID at a time).
- **Why:** Establishes the baseline throughput when there is no batching. Limited by:
  - Database lock contention on the node state row
  - WAL fsync latency (each transaction must durably record the new state)
  - Network round-trip latency
- **Expected range on modern SSD:** 5K–50K TPS per hot node (depending on DB tuning, fsync settings, and network).

### 2. Batch Amortization Ratio

- **Script:** `script_batch.sql` (pgbench) or `script_batch.sql` (ostress)
- **What it does:** Calls `generate_ids(42, 100)` once per transaction (100 IDs at a time).
- **Why:** Tests whether larger batches amortize the lock acquisition and WAL fsync cost.
- **Interpretation:**
  - If `script_batch` TPS × 100 ≈ `script_single` TPS, batching amortizes well (good sign).
  - If the ratio is much lower, there are other bottlenecks (e.g., contention-induced clock advancement).

### 3. Latency at Increasing Concurrency

- **Metric:** p99 (99th percentile) and p95 latencies reported by pgbench/ostress.
- **What to observe:**
  - Does p99 latency grow linearly with concurrency? (Expected.)
  - Does it plateau or grow super-linearly? (May indicate lock contention or cascading wait events.)

### 4. Database Wait Events

**Postgres:**
- Sample `pg_stat_activity` during the benchmark to observe wait events.
- Look for:
  - `lock` events (contention on the state row lock)
  - `io` events (WAL fsync stalls)
  - `CPU` (no stall; good scaling)

**SQL Server:**
- Sample `sys.dm_os_waiting_tasks` during the benchmark to observe wait types.
- Look for:
  - `PAGEIOLATCH_*` (WAL log I/O)
  - `LATCH_*` (internal latching, can indicate state row hotspot)
  - `WRITELOG` (explicit fsync contention)

## Database Setup

### PostgreSQL

Install the schema using the Rust tooling:
```bash
cargo run --bin heeranjid --features postgres -- init \
  --database-url "postgresql://user:pass@localhost:5432/heeranjid_bench"
```

Or manually via psql:
```bash
psql -U user -h localhost -d heeranjid_bench -f sql/postgres/schema.sql
psql -U user -h localhost -d heeranjid_bench -f sql/postgres/functions/generate_heerid.sql
psql -U user -h localhost -d heeranjid_bench -f sql/postgres/functions/generate_ranjid.sql
```

### SQL Server

Install the schema using `sqlcmd`:
```bash
sqlcmd -S localhost -U sa -P "YourPassword" -d heeranjid_bench -i sql/mssql/schema.sql
sqlcmd -S localhost -U sa -P "YourPassword" -d heeranjid_bench -i sql/mssql/procedures/generate_heerid.sql
sqlcmd -S localhost -U sa -P "YourPassword" -d heeranjid_bench -i sql/mssql/procedures/generate_ranjid.sql
```

Ensure `heer_config` row exists:
```sql
-- PostgreSQL
INSERT INTO heer_config (id, epoch) VALUES (1, '2020-01-01'::TIMESTAMP)
ON CONFLICT (id) DO NOTHING;

-- SQL Server
INSERT INTO heer_config (id, epoch) VALUES (1, '2020-01-01');
```

## Running the Benchmarks

### PostgreSQL (pgbench)

```bash
cd benches/pgbench
export DATABASE_URL="postgresql://user:pass@localhost:5432/heeranjid_bench"
export CONCURRENCY=32
export DURATION=60

# Run single-ID benchmark
./run.sh script_single

# Run batch benchmark
./run.sh script_batch
```

**Environment variables:**
- `DATABASE_URL`: Connection string (required; no default).
- `CONCURRENCY`: Number of concurrent connections (`-c` flag). Default: `32`.
- `DURATION`: Test duration in seconds (`-T` flag). Default: `60`.

**Output includes:**
- Main pgbench report (TPS, latencies, transaction counts).
- Sampled `pg_stat_activity` wait events (one sample every ~10 seconds).

### SQL Server (ostress)

```bash
cd benches/ostress
export SERVER="localhost"
export USERNAME="sa"
export PASSWORD="YourPassword"
export DATABASE="heeranjid_bench"
export CONCURRENCY=32
export DURATION=60

# Run single-ID benchmark (requires Windows or wine)
./run.sh script_single

# Run batch benchmark
./run.sh script_batch
```

**Environment variables:**
- `SERVER`: SQL Server host. Default: `localhost`.
- `USERNAME`: Login user. Default: `sa`.
- `PASSWORD`: Login password. Required if using Windows auth is not an option.
- `DATABASE`: Database name. Default: `heeranjid_bench`.
- `CONCURRENCY`: Number of worker threads. Default: `32`.
- `DURATION`: Test duration in seconds. Default: `60`.

**Notes:**
- `ostress.exe` (from SQL Server Management Studio / RML Utilities) is Windows-only.
- On Linux/macOS, use `sqlcmd` as a fallback (see below) or run the script in a Windows environment / via wine.

#### Alternative: sqlcmd Fallback

If `ostress` is unavailable, the script can fall back to `sqlcmd` for simpler sequential testing:

```bash
# Manual fallback (not integrated into run.sh yet)
sqlcmd -S localhost -U sa -P "YourPassword" -d heeranjid_bench \
  -i script_single.sql -o output.txt
```

## Worked Example

**Setup:**
- **DB:** PostgreSQL on modern SSD (AWS gp3 or similar)
- **Node:** node_id = 42 (fixed)
- **Concurrency:** 32 threads
- **Duration:** 60 seconds

**Expected Results:**

| Benchmark | Transactions | TPS | p95 (ms) | p99 (ms) | Notes |
|-----------|--------------|-----|----------|----------|-------|
| single | ~1.8M | ~30K | 1.2 | 2.5 | Baseline; bounded by WAL fsync |
| batch (100) | ~180K | ~3M IDs/sec (30K TPS) | 1.5 | 3.2 | 100× TPS at transaction level; good amortization |

**Wait events (from pg_stat_activity samples):**
- Mostly `NULL` (CPU-bound, not blocked).
- Occasional `io` (WAL fsync, expected).
- Rare `lock` (well-optimized database, minimal lock contention).

**Interpretation:**
- Single-ID throughput of 30K TPS is healthy for a hot node.
- Batch amortization is excellent (100 IDs in nearly the same time as 1).
- No cascading latency as concurrency grows (p99 stays under 5ms).
- This design can support 30K single-ID requests/sec per node without hitting architectural limits.

## Interpreting Results

### High TPS, Low Latency
✓ Design is sound for this workload. Consider scaling horizontally (more nodes) if needed.

### Low TPS, High Latency
Potential causes:
- **I/O latency:** Slow storage (check WAL fsync times with `iostat`).
- **Lock contention:** Database is serializing all writers on the state row (expected, but may indicate need for buffering or higher batch sizes).
- **Network latency:** High RTT to database (try running benchmark on same host).
- **Database misconfiguration:** Check `synchronous_commit` (PostgreSQL) or `RECOVERY_IO_PRIORITY_HIGH` (SQL Server).

### Batch Amortization Low (<50×)
- Batch clock advancement may be hitting the capacity limit per timestamp unit (8K sequences for HeerId, 64K for RanjId).
- Increase batch size further or switch to larger timestamp granularities (RanjId supports microseconds vs HeerId's milliseconds).

### Wait Events Mostly `lock`
- High contention on the state row.
- Expected behavior for a hot node. If unacceptable, consider:
  - Sharding writes across multiple nodes.
  - Implementing a local counter / buffering layer in the application.
  - Pre-allocating larger batches to reduce lock frequency.

## Limitations

These benchmarks test **generator throughput in isolation**. They do not measure:
- End-to-end application latency (includes application logic, deserialization, etc.).
- Real-world skewed workload patterns (most IDs may not hit a single hot node).
- Impact of other concurrent database operations (e.g., other indexes, full table scans).

For production, profile your actual application workload and monitor real metrics (e.g., ID generation latency p99 in your request traces).

## References

- Design doc: [`docs/design/future-tick-behavior.md`](../design/future-tick-behavior.md)
- HeerId spec: [`docs/specification.md`](../specification.md)
- RanjId spec: [`docs/ranjid-specification.md`](../ranjid-specification.md)
