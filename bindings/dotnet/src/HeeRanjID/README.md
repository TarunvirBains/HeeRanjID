# HeeRanjID

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

.NET bindings for HeerRanjId — a Snowflake-style distributed ID system with first-class Postgres and SQL Server support.

The package exposes:

- `HeerId`: compact 64-bit Snowflake-style identifier, stored as `bigint` (Postgres and MSSQL)
- `RanjId`: UUIDv8-compatible 128-bit upgrade format with sub-millisecond precision, stored as `uuid` (Postgres) or `BINARY(16)` (MSSQL)

`RanjId` is stored as `BINARY(16)` on SQL Server rather than `uniqueidentifier` so that the raw big-endian byte layout is preserved and time-ordered index scans work correctly. Use `RanjId.FromBytes(byte[])` to round-trip values read from a `BINARY(16)` column, and `RanjId.ToBytes()` to write them back.

```csharp
using HeeRanjID;

var hid = new HeerId(137438953472L);
Console.WriteLine(hid.TimestampMs);  // 41-bit millisecond timestamp
Console.WriteLine(hid.NodeId);       // 9-bit node identifier
Console.WriteLine(hid.Sequence);     // 13-bit sequence number

var rid = RanjId.Parse("00000000-0000-8000-8007-a120006400c8");
Console.WriteLine(rid.ToGuid());     // Postgres / Guid path
Console.WriteLine(rid.ToBytes());    // SQL Server BINARY(16) path
```

For Entity Framework Core integration — including the provider-aware `UseHeeRanjIdConverters` overload that automatically selects the correct converter for SQL Server — see `HeeRanjID.EFCore`.
