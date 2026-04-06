using System.Data;
using HeeRanjID;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Storage;

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

    /// <summary>
    /// Generates a single HeerId from the database using the given node ID.
    /// </summary>
    public static async Task<HeerId> GenerateHeerIdAsync(
        this DbContext context, int nodeId, CancellationToken ct = default)
    {
        var ids = await context.GenerateHeerIdsAsync(nodeId, 1, ct);
        return ids[0];
    }

    /// <summary>
    /// Generates a batch of HeerIds from the database using the given node ID.
    /// </summary>
    public static async Task<List<HeerId>> GenerateHeerIdsAsync(
        this DbContext context, int nodeId, int count, CancellationToken ct = default)
    {
        if (count <= 0) return [];

        var conn = context.Database.GetDbConnection();
        if (conn.State != ConnectionState.Open)
            await conn.OpenAsync(ct);

        using var cmd = conn.CreateCommand();
        if (context.Database.CurrentTransaction != null)
            cmd.Transaction = context.Database.CurrentTransaction.GetDbTransaction();

        cmd.CommandText = IsMssql(context)
            ? $"EXEC generate_ids @in_node_id = {nodeId}, @requested_count = {count}"
            : $"SELECT id FROM generate_ids({nodeId}, {count})";

        var results = new List<HeerId>(count);
        using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
            results.Add(new HeerId(reader.GetInt64(0)));
        return results;
    }

    /// <summary>
    /// Generates a single RanjId from the database using the given node ID.
    /// </summary>
    public static async Task<RanjId> GenerateRanjIdAsync(
        this DbContext context, int nodeId, CancellationToken ct = default)
    {
        var ids = await context.GenerateRanjIdsAsync(nodeId, 1, ct);
        return ids[0];
    }

    /// <summary>
    /// Generates a batch of RanjIds from the database using the given node ID.
    /// </summary>
    public static async Task<List<RanjId>> GenerateRanjIdsAsync(
        this DbContext context, int nodeId, int count, CancellationToken ct = default)
    {
        if (count <= 0) return [];

        var conn = context.Database.GetDbConnection();
        if (conn.State != ConnectionState.Open)
            await conn.OpenAsync(ct);

        using var cmd = conn.CreateCommand();
        if (context.Database.CurrentTransaction != null)
            cmd.Transaction = context.Database.CurrentTransaction.GetDbTransaction();

        var mssql = IsMssql(context);
        cmd.CommandText = mssql
            ? $"EXEC generate_ranjids @in_node_id = {nodeId}, @requested_count = {count}"
            : $"SELECT id::text FROM generate_ranjids({nodeId}, {count})";

        var results = new List<RanjId>(count);
        using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            if (mssql)
            {
                // MSSQL returns binary (varbinary); convert to Guid bytes then RanjId
                var bytes = (byte[])reader.GetValue(0);
                results.Add(RanjId.FromGuid(new Guid(bytes)));
            }
            else
            {
                results.Add(RanjId.Parse(reader.GetString(0)));
            }
        }
        return results;
    }

    private static bool IsMssql(DbContext context) =>
        context.Database.ProviderName?.Contains("SqlServer", StringComparison.OrdinalIgnoreCase) == true;
}
