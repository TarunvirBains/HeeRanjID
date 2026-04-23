#!/bin/bash
# ostress runner for HeerId and RanjId hot-node benchmarks.
# Runs ostress.exe (SQL Server RML Utilities) with a custom script.
# Falls back to sqlcmd if ostress is unavailable (for basic sequential testing).

set -euo pipefail

# Default values
SERVER="${SERVER:-localhost}"
USERNAME="${USERNAME:-sa}"
DATABASE="${DATABASE:-heeranjid_bench}"
CONCURRENCY="${CONCURRENCY:-32}"
DURATION="${DURATION:-60}"

if [[ $# -lt 1 ]]; then
    cat >&2 << 'EOF'
Usage: ./run.sh <script_name>

Required environment variables:
    SERVER       - SQL Server host (default: localhost)
    USERNAME     - Login user (default: sa)
    DATABASE     - Database name (default: heeranjid_bench)
    PASSWORD     - Login password (if using SQL auth; omit for Windows auth)

Optional environment variables:
    CONCURRENCY  - Number of worker threads (default: 32)
    DURATION     - Test duration in seconds (default: 60)

Examples:
    export PASSWORD="MyPassword"
    ./run.sh script_single
    ./run.sh script_batch

    SERVER=prod-sql.example.com CONCURRENCY=64 ./run.sh script_single
EOF
    exit 1
fi

SCRIPT_NAME="$1"
SCRIPT_PATH="$(dirname "$0")/${SCRIPT_NAME}.sql"

if [[ ! -f "$SCRIPT_PATH" ]]; then
    echo "Error: Script not found: $SCRIPT_PATH" >&2
    exit 1
fi

echo "=== SQL Server Benchmark Configuration ==="
echo "Server: $SERVER"
echo "Database: $DATABASE"
echo "Username: $USERNAME"
echo "Script: $SCRIPT_PATH"
echo "Concurrency: $CONCURRENCY"
echo "Duration: ${DURATION}s"
echo ""

# Build sqlcmd auth args
SQLCMD_AUTH=()
if [[ -n "${PASSWORD:-}" ]]; then
    SQLCMD_AUTH=("-P" "$PASSWORD")
else
    # Assume Windows auth; add -E flag
    SQLCMD_AUTH=("-E")
fi

# Check if ostress.exe is available
if command -v ostress.exe &>/dev/null || command -v ostress &>/dev/null; then
    OSTRESS_CMD="ostress.exe"
    if ! command -v "$OSTRESS_CMD" &>/dev/null; then
        OSTRESS_CMD="ostress"
    fi

    echo "=== Using ostress.exe (RML Utilities) ==="
    echo ""

    # ostress flags:
    #   -S <server>          - Server name
    #   -d <database>        - Database
    #   -U <username>        - Login
    #   -P <password>        - Password (omitted for Windows auth)
    #   -i <input_file>      - Input SQL script
    #   -n <num_threads>     - Number of threads
    #   -r <num_requests>    - Requests per thread (total ops ≈ threads × requests)
    #   -q                   - Quiet mode (less verbose)
    #
    # Note: ostress doesn't have a direct duration flag; instead use -r to set total iterations.
    # Estimate: 1000 requests per thread over ~60s at 30K TPS ≈ 32 threads × 1000 = 32K ops in ~1s.
    # For longer runs, scale up -r. Formula: requests ≈ (target_tps / num_threads) * duration.
    # For a 60s run at ~30K TPS with 32 threads: 30000 / 32 * 60 ≈ 56,250 requests per thread.

    REQUESTS_PER_THREAD=$(( (30000 / CONCURRENCY) * DURATION ))

    # Construct ostress command
    OSTRESS_CMD_ARGS=(
        "-S" "$SERVER"
        "-d" "$DATABASE"
        "-U" "$USERNAME"
        "${SQLCMD_AUTH[@]}"
        "-i" "$SCRIPT_PATH"
        "-n" "$CONCURRENCY"
        "-r" "$REQUESTS_PER_THREAD"
        "-q"
    )

    echo "ostress.exe ${OSTRESS_CMD_ARGS[*]}"
    echo ""

    "$OSTRESS_CMD" "${OSTRESS_CMD_ARGS[@]}"

else
    echo "=== ostress.exe not found; falling back to sqlcmd (sequential mode) ==="
    echo "Note: sqlcmd runs sequentially, not with concurrency. For concurrent testing, install ostress.exe from SQL Server Management Studio."
    echo ""

    # Build output file name for results
    OUTPUT_FILE="/tmp/heeranjid_bench_${SCRIPT_NAME}_$(date +%s).txt"

    # Run sqlcmd sequentially; it will iterate DURATION times or until EOF
    # Note: sqlcmd doesn't have a built-in duration or concurrency flag; this is a simple fallback.
    # For real benchmarking, use ostress.exe on Windows.

    SQLCMD_CMD_ARGS=(
        "-S" "$SERVER"
        "-U" "$USERNAME"
        "${SQLCMD_AUTH[@]}"
        "-d" "$DATABASE"
        "-i" "$SCRIPT_PATH"
        "-o" "$OUTPUT_FILE"
    )

    echo "sqlcmd ${SQLCMD_CMD_ARGS[*]}"
    echo ""

    sqlcmd "${SQLCMD_CMD_ARGS[@]}"

    echo ""
    echo "Results written to: $OUTPUT_FILE"
    echo "(Note: This was a sequential run via sqlcmd. For concurrent testing, use ostress.exe on Windows.)"
fi

echo ""
echo "=== Benchmark complete ==="
