using System.Reflection;

namespace HeeRanjID;

/// <summary>
/// Provides access to the embedded SQL migration scripts.
/// </summary>
public static class SqlHelper
{
    private static readonly Assembly ThisAssembly = typeof(SqlHelper).Assembly;

    /// <summary>
    /// Returns the full install SQL (schema + all functions) for Postgres,
    /// concatenated into a single script ready for execution.
    /// </summary>
    public static string GetInstallSql()
        => string.Join("\n",
            GetSchemaSql(),
            GetSessionSql(),
            GetGenerateHeerIdSql(),
            GetGenerateRanjIdSql());

    /// <summary>
    /// Returns the schema-only SQL for Postgres.
    /// </summary>
    public static string GetSchemaSql()
        => ReadResource("HeeRanjID.Sql.postgres.schema.sql");

    /// <summary>
    /// Returns the seed SQL for Postgres.
    /// </summary>
    public static string GetSeedSql()
        => ReadResource("HeeRanjID.Sql.postgres.seed.sql");

    /// <summary>
    /// Returns the generate_heerid function SQL.
    /// </summary>
    public static string GetGenerateHeerIdSql()
        => ReadResource("HeeRanjID.Sql.postgres.functions.generate_heerid.sql");

    /// <summary>
    /// Returns the generate_ranjid function SQL.
    /// </summary>
    public static string GetGenerateRanjIdSql()
        => ReadResource("HeeRanjID.Sql.postgres.functions.generate_ranjid.sql");

    /// <summary>
    /// Returns the session function SQL.
    /// </summary>
    public static string GetSessionSql()
        => ReadResource("HeeRanjID.Sql.postgres.functions.session.sql");

    /// <summary>
    /// Returns all available SQL resource names.
    /// </summary>
    public static string[] GetResourceNames()
        => ThisAssembly.GetManifestResourceNames()
            .Where(n => n.EndsWith(".sql", StringComparison.OrdinalIgnoreCase))
            .ToArray();

    private static string ReadResource(string name)
    {
        using var stream = ThisAssembly.GetManifestResourceStream(name)
            ?? throw new InvalidOperationException($"Embedded resource '{name}' not found.");
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }
}
