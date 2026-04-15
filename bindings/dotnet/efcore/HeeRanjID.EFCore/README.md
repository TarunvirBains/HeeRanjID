# HeeRanjID.EFCore

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Entity Framework Core integration for HeeRanjId — a Snowflake-style distributed ID system.

This package provides value converters and a `SaveChanges` interceptor that automatically assigns `HeerId` and `RanjId` values to new entities before they are saved.

```csharp
// Program.cs
services.AddHeeRanjId(new HeeRanjIdOptions { NodeId = 1 });
// or read from NODE_ID environment variable:
// services.AddHeeRanjId(HeeRanjIdOptions.FromEnvironment());

services.AddDbContext<AppDbContext>((sp, options) =>
    options
        .UseNpgsql(connectionString)
        .AddInterceptors(sp.GetRequiredService<HeeRanjIdSaveChangesInterceptor>()));

// AppDbContext.cs
protected override void OnModelCreating(ModelBuilder modelBuilder)
{
    modelBuilder.UseHeeRanjIdConverters();
}
```

Any entity property of type `HeerId` or `RanjId` with a default value (`0` or `default`) will have an ID assigned automatically when `SaveChanges` or `SaveChangesAsync` is called.

For the core types, see `HeeRanjID`.
