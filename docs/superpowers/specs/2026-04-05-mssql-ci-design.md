# MSSQL CI Design

## Goal

Add MSSQL integration testing to the existing Python CI job alongside the Postgres tests.

## Design

The Python CI job gets a second database service (MSSQL) alongside the existing Postgres service. Both test suites run in the same job.

### Service Container

```yaml
services:
  postgres:
    # ... existing config ...
  mssql:
    image: mcr.microsoft.com/mssql/server:2022-latest
    env:
      ACCEPT_EULA: Y
      MSSQL_SA_PASSWORD: HeeRanjID_Test1
    options: >-
      --health-cmd "/opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -P HeeRanjID_Test1 -C -Q 'SELECT 1'"
      --health-interval 10s
      --health-timeout 5s
      --health-retries 10
```

### ODBC Driver Installation

Before running MSSQL tests, install the ODBC driver in the Python container:

```yaml
- name: Install ODBC driver
  run: |
    apt-get update
    ACCEPT_EULA=Y apt-get install -y curl gnupg2
    curl -fsSL https://packages.microsoft.com/keys/microsoft.asc | gpg --dearmor -o /usr/share/keyrings/microsoft-prod.gpg
    echo "deb [signed-by=/usr/share/keyrings/microsoft-prod.gpg] https://packages.microsoft.com/debian/12/prod bookworm main" > /etc/apt/sources.list.d/mssql-release.list
    apt-get update
    ACCEPT_EULA=Y apt-get install -y msodbcsql18 unixodbc-dev
    pip install pyodbc
```

### Test Step

```yaml
- name: Run MSSQL integration tests
  run: pytest bindings/python/django/tests/test_mssql_integration.py -v
  env:
    MSSQL_URL: "DRIVER={ODBC Driver 18 for SQL Server};SERVER=mssql,1433;UID=sa;PWD=HeeRanjID_Test1;TrustServerCertificate=yes"
```

### Local Script

Add to `scripts/check.sh` (optional, behind env var check):

```bash
if [ -n "$MSSQL_URL" ]; then
    echo "=== MSSQL integration tests ==="
    pytest bindings/python/django/tests/test_mssql_integration.py -v
fi
```

## What's NOT in scope

- MSSQL tests for TypeScript or .NET (future work)
- MSSQL service in the Rust CI job (Rust sqlx doesn't have MSSQL support)
