# HeeRanjID

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

.NET bindings for HeerRanjId — a Snowflake-style distributed ID system.

The package exposes:

- `HeerId`: compact 64-bit Snowflake-style identifier, stored as `bigint`
- `RanjId`: UUIDv8-compatible 128-bit upgrade format with sub-millisecond precision, stored as `uuid`

```csharp
using HeeRanjID;

var hid = new HeerId(137438953472L);
Console.WriteLine(hid.TimestampMs);  // 41-bit millisecond timestamp
Console.WriteLine(hid.NodeId);       // 9-bit node identifier
Console.WriteLine(hid.Sequence);     // 13-bit sequence number

var rid = RanjId.Parse("00000000-0000-8000-8007-a120006400c8");
Console.WriteLine(rid.ToGuid());
```

For Entity Framework Core integration, see `HeeRanjID.EFCore`.
