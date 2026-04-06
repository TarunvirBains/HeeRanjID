using Microsoft.Extensions.DependencyInjection;

namespace HeeRanjID.EFCore;

public static class ServiceCollectionExtensions
{
    /// <summary>
    /// Registers <see cref="HeeRanjIdSaveChangesInterceptor"/> as a singleton so EF Core
    /// can resolve it from the DI container.
    ///
    /// After calling this, wire up the interceptor in your DbContext setup:
    /// <code>
    /// services.AddDbContext&lt;AppDbContext&gt;((sp, options) =>
    ///     options.AddInterceptors(sp.GetRequiredService&lt;HeeRanjIdSaveChangesInterceptor&gt;()));
    /// </code>
    /// </summary>
    public static IServiceCollection AddHeeRanjId(
        this IServiceCollection services,
        HeeRanjIdOptions options)
    {
        services.AddSingleton(options);
        services.AddSingleton<HeeRanjIdSaveChangesInterceptor>();
        return services;
    }

    /// <inheritdoc cref="AddHeeRanjId(IServiceCollection, HeeRanjIdOptions)"/>
    public static IServiceCollection AddHeeRanjId(this IServiceCollection services)
        => services.AddHeeRanjId(new HeeRanjIdOptions());
}
