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

        if (IsMssql(context))
        {
            cmd.CommandText = "EXEC generate_ids @in_node_id = @in_node_id, @requested_count = @requested_count";

            var nodeParam = cmd.CreateParameter();
            nodeParam.ParameterName = "@in_node_id";
            nodeParam.DbType = DbType.Int32;
            nodeParam.Value = nodeId;
            cmd.Parameters.Add(nodeParam);

            var countParam = cmd.CreateParameter();
            countParam.ParameterName = "@requested_count";
            countParam.DbType = DbType.Int32;
            countParam.Value = count;
            cmd.Parameters.Add(countParam);
        }
        else
        {
            cmd.CommandText = "SELECT id FROM generate_ids(@p_node_id, @p_count)";

            var nodeParam = cmd.CreateParameter();
            nodeParam.ParameterName = "@p_node_id";
            nodeParam.DbType = DbType.Int32;
            nodeParam.Value = nodeId;
            cmd.Parameters.Add(nodeParam);

            var countParam = cmd.CreateParameter();
            countParam.ParameterName = "@p_count";
            countParam.DbType = DbType.Int32;
            countParam.Value = count;
            cmd.Parameters.Add(countParam);
        }

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
                // MSSQL returns BINARY(16) — raw big-endian bytes.  Use FromBytes
                // directly to preserve sort order; routing through Guid would
                // apply mixed-endian swizzle and corrupt the byte sequence.
                var bytes = (byte[])reader.GetValue(0);
                results.Add(RanjId.FromBytes(bytes));
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
