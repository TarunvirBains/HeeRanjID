using HeeRanjID;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata;

namespace HeeRanjID.EFCore;

/// <summary>
/// Extension methods for configuring HeerId and RanjId in EF Core models.
/// </summary>
public static class ModelBuilderExtensions
{
    /// <summary>
    /// Registers value converters for all <see cref="HeerId"/> and
    /// <see cref="RanjId"/> properties discovered in the model.
    /// Uses Guid-based storage (non-SQL-Server default).
    /// </summary>
    /// <remarks>
    /// When targeting SQL Server, pass the provider name so that RanjId
    /// properties receive the correct <c>BINARY(16)</c> converter and column
    /// type annotation — see the overload
    /// <see cref="UseHeeRanjIdConverters(ModelBuilder, string?)"/>.
    /// </remarks>
    public static ModelBuilder UseHeeRanjIdConverters(this ModelBuilder modelBuilder)
        => modelBuilder.UseHeeRanjIdConverters(providerName: null);

    /// <summary>
    /// Registers value converters for all <see cref="HeerId"/> and
    /// <see cref="RanjId"/> properties discovered in the model, with
    /// provider-aware storage for SQL Server.
    /// </summary>
    /// <param name="modelBuilder">The model builder to configure.</param>
    /// <param name="providerName">
    /// The EF Core provider name, typically obtained from
    /// <c>Database.ProviderName</c> inside <c>OnModelCreating</c>.
    /// When this value contains <c>"SqlServer"</c> (case-insensitive) each
    /// <see cref="RanjId"/> property is configured with
    /// <see cref="RanjIdMssqlValueConverter"/> and
    /// <c>HasColumnType("binary(16)")</c>, preserving big-endian sort order.
    /// For all other providers, <see cref="RanjIdValueConverter"/> (Guid) is used.
    /// Passing <c>null</c> selects the non-SQL-Server path.
    /// </param>
    public static ModelBuilder UseHeeRanjIdConverters(
        this ModelBuilder modelBuilder,
        string? providerName)
    {
        bool isSqlServer = providerName?.Contains(
            "SqlServer", StringComparison.OrdinalIgnoreCase) == true;

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
                    if (isSqlServer)
                    {
                        property.SetValueConverter(new RanjIdMssqlValueConverter());
                        property.SetColumnType("binary(16)");
                    }
                    else
                    {
                        property.SetValueConverter(new RanjIdValueConverter());
                    }
                }
            }
        }

        return modelBuilder;
    }
}
