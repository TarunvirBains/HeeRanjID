-- ostress script: batch ID generation (HeerId)
-- Executes EXEC generate_ids @in_node_id = 42, @requested_count = 100 once per iteration.
-- Measures amortization of lock and transaction log fsync cost across 100 IDs.

EXEC generate_ids @in_node_id = 42, @requested_count = 100;
