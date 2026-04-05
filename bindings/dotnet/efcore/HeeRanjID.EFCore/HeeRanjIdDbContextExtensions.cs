using Microsoft.EntityFrameworkCore;

namespace HeeRanjID.EFCore;

public static class HeeRanjIdDbContextExtensions
{
    /// <summary>
    /// Installs the HeeRanjID schema, functions, and seed data.
    /// Calls heer_configure() to bake in epoch and precision.
    /// </summary>
    public static void InstallHeeRanjId(this DbContext context, string backend = "postgres")
    {
        var sql = SqlHelper.GetInstallSql(backend);
        context.Database.ExecuteSqlRaw(sql);

        var seed = SqlHelper.GetSeedSql(backend);
        context.Database.ExecuteSqlRaw(seed);

        // Configure to bake in epoch/precision
        if (backend == "mssql")
            context.Database.ExecuteSqlRaw("EXEC heer_configure");
        else
            context.Database.ExecuteSqlRaw("SELECT heer_configure()");
    }
}
