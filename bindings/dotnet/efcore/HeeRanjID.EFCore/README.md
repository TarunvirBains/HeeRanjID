# HeeRanjID.EFCore

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Entity Framework Core integration for HeeRanjId — a Snowflake-style distributed ID system with first-class Postgres and SQL Server support.

This package provides value converters and a `SaveChanges` interceptor that automatically assigns `HeerId` and `RanjId` values to new entities before they are saved.

## Value converters

`UseHeeRanjIdConverters` has two overloads:

- `modelBuilder.UseHeeRanjIdConverters()` — registers the Guid-based `RanjIdValueConverter` for all `RanjId` properties. Correct for Postgres, SQLite, and other non-SQL-Server providers where `RanjId` is stored as `uuid` / `text`.
- `modelBuilder.UseHeeRanjIdConverters(providerName)` — registers a provider-aware converter. When `providerName` contains `"SqlServer"` (case-insensitive), each `RanjId` property receives `RanjIdMssqlValueConverter` and the column type is set to `binary(16)`. For all other provider names, the Guid-based path is used. Pass `Database.ProviderName` from inside `OnModelCreating` to select automatically.

`HeerId` always maps to `bigint` regardless of provider.

### Why `BINARY(16)` for SQL Server

SQL Server's `uniqueidentifier` type stores bytes in mixed-endian order (first three groups little-endian, last two big-endian), which scrambles the high-bit timestamp prefix that makes `RanjId` time-ordered. Storing as `BINARY(16)` preserves the raw big-endian byte layout so that time-ordered index scans work correctly on SQL Server without per-vendor workarounds.

## Usage

```csharp
// Program.cs
services.AddHeeRanjId(new HeeRanjIdOptions { NodeId = 1 });
// or read from NODE_ID environment variable:
// services.AddHeeRanjId(HeeRanjIdOptions.FromEnvironment());

services.AddDbContext<AppDbContext>((sp, options) =>
    options
        .UseNpgsql(connectionString)                    // Postgres
        // .UseSqlServer(connectionString)              // SQL Server
        .AddInterceptors(sp.GetRequiredService<HeeRanjIdSaveChangesInterceptor>()));

// AppDbContext.cs
protected override void OnModelCreating(ModelBuilder modelBuilder)
{
    // Non-SQL-Server (Postgres, SQLite, etc.) — Guid-based RanjId converter:
    modelBuilder.UseHeeRanjIdConverters();

    // SQL Server — automatically selects BINARY(16) converter for RanjId:
    // modelBuilder.UseHeeRanjIdConverters(Database.ProviderName);
}
```

Any entity property of type `HeerId` or `RanjId` with a default value (`0` or `default`) will have an ID assigned automatically when `SaveChanges` or `SaveChangesAsync` is called.

For the core types and `RanjId.FromBytes` / `RanjId.ToBytes`, see `HeeRanjID`.

### Breaking change — SQL Server BINARY(16) (v0.5.0)

Before v0.5.0, the EF Core integration mapped `RanjId` to `uniqueidentifier` on SQL
Server. Because .NET's `Guid` type stores bytes in mixed-endian order (groups 0–3,
4–5, and 6–7 stored little-endian), the timestamp prefix was silently scrambled on
every read and write — breaking the time-ordered sort guarantee that is core to
`RanjId`'s design.

v0.5.0 fixes this by mapping `RanjId` to `BINARY(16)` (raw big-endian bytes) on SQL
Server. **This is a breaking schema change for anyone who ran a .NET build that
targeted SQL Server before v0.5.0.** Existing `uniqueidentifier` columns are
type-incompatible with the new converter and will cause a `FormatException` or cast
failure at query time; a manual schema migration is required before upgrading:

```sql
-- Illustrative type change only — does NOT preserve existing data byte order.
-- A full data-preserving migration script will accompany the first NuGet publish.
ALTER TABLE dbo.MyTable ALTER COLUMN Id BINARY(16) NOT NULL;
```

A full migration script and step-by-step guide (including the data-preserving byte
reorder) will accompany the first NuGet publish. If you need migration help before
then, file an issue at https://github.com/TarunvirBains/HeeRanjID/issues.
