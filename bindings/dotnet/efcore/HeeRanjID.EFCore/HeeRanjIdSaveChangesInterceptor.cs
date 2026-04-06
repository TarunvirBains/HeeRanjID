using HeeRanjID;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Diagnostics;

namespace HeeRanjID.EFCore;

/// <summary>
/// EF Core interceptor that automatically generates HeeRanjID values before
/// entities are saved. Assign a HeerIdField or RanjIdField property its
/// default (0 or <see langword="default"/>) and the interceptor will replace
/// it with a database-generated ID in the same operation.
/// </summary>
public class HeeRanjIdSaveChangesInterceptor : SaveChangesInterceptor
{
    private readonly HeeRanjIdOptions _options;

    public HeeRanjIdSaveChangesInterceptor(HeeRanjIdOptions options)
    {
        _options = options;
    }

    public HeeRanjIdSaveChangesInterceptor() : this(new HeeRanjIdOptions()) { }

    public override async ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData,
        InterceptionResult<int> result,
        CancellationToken cancellationToken = default)
    {
        if (eventData.Context is not null)
            await AssignMissingIdsAsync(eventData.Context, cancellationToken);
        return result;
    }

    public override InterceptionResult<int> SavingChanges(
        DbContextEventData eventData,
        InterceptionResult<int> result)
    {
        if (eventData.Context is not null)
            AssignMissingIdsAsync(eventData.Context, CancellationToken.None).GetAwaiter().GetResult();
        return result;
    }

    private async Task AssignMissingIdsAsync(DbContext context, CancellationToken ct)
    {
        // Collect all Added entries with unset HeerId / RanjId properties.
        var heerFields = new List<(Microsoft.EntityFrameworkCore.ChangeTracking.EntityEntry Entry, string PropertyName)>();
        var ranjFields = new List<(Microsoft.EntityFrameworkCore.ChangeTracking.EntityEntry Entry, string PropertyName)>();

        foreach (var entry in context.ChangeTracker.Entries().Where(e => e.State == EntityState.Added))
        {
            foreach (var prop in entry.Metadata.GetProperties())
            {
                if (prop.ClrType == typeof(HeerId))
                {
                    var current = entry.Property(prop.Name).CurrentValue;
                    if (current is HeerId hid && hid.Value == 0)
                        heerFields.Add((entry, prop.Name));
                }
                else if (prop.ClrType == typeof(RanjId))
                {
                    var current = entry.Property(prop.Name).CurrentValue;
                    if (current is RanjId rid && rid.Equals(default(RanjId)))
                        ranjFields.Add((entry, prop.Name));
                }
            }
        }

        if (heerFields.Count > 0)
        {
            var ids = await context.GenerateHeerIdsAsync(_options.NodeId, heerFields.Count, ct);
            for (int i = 0; i < heerFields.Count; i++)
                heerFields[i].Entry.Property(heerFields[i].PropertyName).CurrentValue = ids[i];
        }

        if (ranjFields.Count > 0)
        {
            var ids = await context.GenerateRanjIdsAsync(_options.NodeId, ranjFields.Count, ct);
            for (int i = 0; i < ranjFields.Count; i++)
                ranjFields[i].Entry.Property(ranjFields[i].PropertyName).CurrentValue = ids[i];
        }
    }
}
