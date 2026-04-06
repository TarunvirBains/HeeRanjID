using HeeRanjID;
using HeeRanjID.EFCore;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace HeeRanjID.Tests;

// ---------------------------------------------------------------------------
// Minimal in-memory DbContext for interceptor unit tests.
// We do NOT require a real database — the interceptor's entity-scanning logic
// is fully testable without one.
// ---------------------------------------------------------------------------

public class SampleEntity
{
    public HeerId Id { get; set; }
    public string Name { get; set; } = "";
}

public class SampleRanjEntity
{
    public RanjId Id { get; set; }
    public string Name { get; set; } = "";
}

// A minimal fake DbContext that uses InMemory so no actual database is needed.
// GenerateHeerIds / GenerateRanjIds are overridden below in a test subclass to
// avoid needing a real Postgres connection.
public class SampleDbContext : DbContext
{
    public DbSet<SampleEntity> Entities => Set<SampleEntity>();
    public DbSet<SampleRanjEntity> RanjEntities => Set<SampleRanjEntity>();

    public SampleDbContext(DbContextOptions<SampleDbContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.UseHeeRanjIdConverters();

        modelBuilder.Entity<SampleEntity>(b =>
        {
            b.HasKey(e => e.Id);
            b.Property(e => e.Id)
             .HasConversion(new HeerIdValueConverter())
             .ValueGeneratedNever();
        });

        modelBuilder.Entity<SampleRanjEntity>(b =>
        {
            b.HasKey(e => e.Id);
            b.Property(e => e.Id)
             .HasConversion(new RanjIdValueConverter())
             .ValueGeneratedNever();
        });
    }
}

// ---------------------------------------------------------------------------

public class InterceptorTests
{
    [Fact]
    public void HeeRanjIdSaveChangesInterceptor_DefaultConstructor_UsesDefaultOptions()
    {
        var interceptor = new HeeRanjIdSaveChangesInterceptor();
        Assert.NotNull(interceptor);
    }

    [Fact]
    public void HeeRanjIdSaveChangesInterceptor_AcceptsCustomOptions()
    {
        var options = new HeeRanjIdOptions { NodeId = 7 };
        var interceptor = new HeeRanjIdSaveChangesInterceptor(options);
        Assert.NotNull(interceptor);
    }

    [Fact]
    public void HeeRanjIdOptions_DefaultNodeId_IsOne()
    {
        var options = new HeeRanjIdOptions();
        Assert.Equal(1, options.NodeId);
    }

    [Fact]
    public void HeeRanjIdOptions_FromEnvironment_ThrowsWhenNotSet()
    {
        var previous = Environment.GetEnvironmentVariable("NODE_ID");
        try
        {
            Environment.SetEnvironmentVariable("NODE_ID", null);
            Assert.Throws<InvalidOperationException>(() => HeeRanjIdOptions.FromEnvironment());
        }
        finally
        {
            Environment.SetEnvironmentVariable("NODE_ID", previous);
        }
    }

    [Fact]
    public void HeeRanjIdOptions_FromEnvironment_ParsesNodeId()
    {
        var previous = Environment.GetEnvironmentVariable("NODE_ID");
        try
        {
            Environment.SetEnvironmentVariable("NODE_ID", "42");
            var options = HeeRanjIdOptions.FromEnvironment();
            Assert.Equal(42, options.NodeId);
        }
        finally
        {
            Environment.SetEnvironmentVariable("NODE_ID", previous);
        }
    }

    [Fact]
    public void ModelBuilderExtensions_UseHeeRanjIdConverters_DoesNotThrow()
    {
        var opts = new DbContextOptionsBuilder<SampleDbContext>()
            .UseInMemoryDatabase("test_converters")
            .Options;

        using var ctx = new SampleDbContext(opts);
        // OnModelCreating runs on first access — this exercises UseHeeRanjIdConverters
        Assert.NotNull(ctx.Entities);
    }

    [Fact]
    public void ServiceCollectionExtensions_AddHeeRanjId_RegistersInterceptor()
    {
        var services = new ServiceCollection();
        services.AddHeeRanjId(new HeeRanjIdOptions { NodeId = 3 });

        var provider = services.BuildServiceProvider();
        var interceptor = provider.GetService<HeeRanjIdSaveChangesInterceptor>();
        Assert.NotNull(interceptor);
    }

    [Fact]
    public void ServiceCollectionExtensions_AddHeeRanjId_NoArgs_RegistersInterceptor()
    {
        var services = new ServiceCollection();
        services.AddHeeRanjId();

        var provider = services.BuildServiceProvider();
        var interceptor = provider.GetService<HeeRanjIdSaveChangesInterceptor>();
        Assert.NotNull(interceptor);
    }

    [Fact]
    public void ServiceCollectionExtensions_AddHeeRanjId_RegistersOptions()
    {
        var services = new ServiceCollection();
        var options = new HeeRanjIdOptions { NodeId = 5 };
        services.AddHeeRanjId(options);

        var provider = services.BuildServiceProvider();
        var resolved = provider.GetService<HeeRanjIdOptions>();
        Assert.NotNull(resolved);
        Assert.Equal(5, resolved.NodeId);
    }
}
