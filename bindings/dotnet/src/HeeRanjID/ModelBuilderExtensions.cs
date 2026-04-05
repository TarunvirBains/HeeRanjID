using Microsoft.EntityFrameworkCore;

namespace HeeRanjID;

/// <summary>
/// Extension methods for configuring HeerId and RanjId in EF Core models.
/// </summary>
public static class ModelBuilderExtensions
{
    /// <summary>
    /// Registers value converters for all HeerId and RanjId properties
    /// discovered in the model.
    /// </summary>
    public static ModelBuilder UseHeeRanjIdConverters(this ModelBuilder modelBuilder)
    {
        foreach (var entityType in modelBuilder.Model.GetEntityTypes())
        {
            foreach (var property in entityType.GetProperties())
            {
                if (property.ClrType == typeof(HeerId))
                {
                    property.SetValueConverter(new HeerIdValueConverter());
                }
                else if (property.ClrType == typeof(RanjId))
                {
                    property.SetValueConverter(new RanjIdValueConverter());
                }
            }
        }

        return modelBuilder;
    }
}
