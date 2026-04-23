-- pgbench script: single-ID generation (HeerId)
-- Calls generate_id(42) once per transaction.
-- Forces contention on a single fixed node to measure throughput ceiling.

SELECT generate_id(42);
