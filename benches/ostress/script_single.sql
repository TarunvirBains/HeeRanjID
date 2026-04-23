-- ostress script: single-ID generation (HeerId)
-- Executes EXEC generate_id @in_node_id = 42 once per iteration.
-- Forces contention on a single fixed node to measure throughput ceiling.

EXEC generate_id @in_node_id = 42;
