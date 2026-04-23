#!/bin/bash
# pgbench runner for HeerId and RanjId hot-node benchmarks.
# Runs pgbench with a custom script and samples pg_stat_activity wait events.

set -euo pipefail

# Default values
CONCURRENCY="${CONCURRENCY:-32}"
DURATION="${DURATION:-60}"

# Validate environment
if [[ -z "${DATABASE_URL:-}" ]]; then
    cat >&2 << 'EOF'
Error: DATABASE_URL not set.

Usage:
    export DATABASE_URL="postgresql://user:pass@localhost:5432/heeranjid_bench"
    export CONCURRENCY=32  # optional, default: 32
    export DURATION=60     # optional, default: 60
    ./run.sh script_single
    ./run.sh script_batch

Examples:
    ./run.sh script_single
    CONCURRENCY=64 DURATION=120 ./run.sh script_batch
EOF
    exit 1
fi

if [[ $# -lt 1 ]]; then
    echo "Usage: ./run.sh <script_name>" >&2
    echo "  script_name: script_single or script_batch" >&2
    exit 1
fi

SCRIPT_NAME="$1"
SCRIPT_PATH="$(dirname "$0")/${SCRIPT_NAME}.sql"

if [[ ! -f "$SCRIPT_PATH" ]]; then
    echo "Error: Script not found: $SCRIPT_PATH" >&2
    exit 1
fi

# Parse DATABASE_URL into components using regex
# Format: postgresql://[user[:password]@]host[:port][/dbname]
if [[ $DATABASE_URL =~ ^postgresql://([^@]*@)?([^:/]+)(:([0-9]+))?(/([^?]*))?$ ]]; then
    USERPASS="${BASH_REMATCH[1]%@}"
    HOST="${BASH_REMATCH[2]}"
    PORT="${BASH_REMATCH[4]:-5432}"
    DBNAME="${BASH_REMATCH[6]}"
else
    echo "Error: Unable to parse DATABASE_URL. Expected format: postgresql://[user:pass@]host[:port][/dbname]" >&2
    exit 1
fi

# Additional pgbench options
JOBS="${JOBS:-8}"  # -j flag for parallel jobs (usually threads/2 or threads/4)

echo "=== pgbench Configuration ==="
echo "Host: $HOST"
echo "Port: $PORT"
echo "Database: $DBNAME"
echo "Script: $SCRIPT_PATH"
echo "Concurrency: $CONCURRENCY"
echo "Jobs: $JOBS"
echo "Duration: ${DURATION}s"
echo ""

# Function to sample pg_stat_activity in background
sample_wait_events() {
    local psql_opts=("-h" "$HOST" "-p" "$PORT" "-U" "${USERPASS%:*}" "-d" "$DBNAME" "-t" "-c")

    # Give pgbench a moment to start
    sleep 2

    echo "=== Sampling pg_stat_activity wait events every ~10 seconds ==="
    while sleep 10; do
        # Check if pgbench is still running
        if ! pgrep -f "pgbench.*$SCRIPT_PATH" > /dev/null 2>&1; then
            break
        fi

        # Sample wait events: show pid, app_name, wait_event_type, and wait_event
        PGPASSWORD="${USERPASS#*:}" psql "${psql_opts[@]}" \
            "SELECT now()::time, pid, application_name, COALESCE(wait_event_type, 'CPU') as wait_type, COALESCE(wait_event, '-') as wait_event FROM pg_stat_activity WHERE application_name LIKE '%pgbench%' ORDER BY pid;" \
            2>/dev/null || true
        echo ""
    done
}

# Start wait event sampling in background
sample_wait_events &
SAMPLER_PID=$!
trap "kill $SAMPLER_PID 2>/dev/null || true" EXIT

# Run pgbench
pgbench \
    -h "$HOST" \
    -p "$PORT" \
    -U "${USERPASS%:*}" \
    -d "$DBNAME" \
    -f "$SCRIPT_PATH" \
    -c "$CONCURRENCY" \
    -j "$JOBS" \
    -T "$DURATION" \
    --progress=10 \
    -r

echo ""
echo "=== Benchmark complete ==="
