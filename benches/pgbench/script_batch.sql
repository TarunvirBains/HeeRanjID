-- pgbench script: batch ID generation (HeerId)
-- Calls generate_ids(42, 100) once per transaction.
-- Measures amortization of lock and WAL fsync cost across 100 IDs.

SELECT generate_ids(42, 100);
