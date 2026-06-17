using System;
using System.Reflection;

namespace HeeRanjID;

/// <summary>
/// Provides access to the SQL migration scripts.
/// Reads from a sql/ directory alongside the assembly.
/// </summary>
public static class SqlHelper
{
    private static string? _sqlBasePath;

    /// <summary>
    /// Sets the base path where SQL files are located.
    /// If not set, defaults to a sql/ directory alongside the assembly.
    /// </summary>
    public static string SqlBasePath
    {
        get => _sqlBasePath ?? Path.Combine(
            Path.GetDirectoryName(typeof(SqlHelper).Assembly.Location)!,
            "sql");
        set => _sqlBasePath = value;
    }

    public static string GetInstallSql(string backend = "postgres")
        => string.Join("\n",
            GetSchemaSql(backend),
            GetSessionSql(backend),
            GetGenerateHeerIdSql(backend),
            GetGenerateRanjIdSql(backend));

    public static string GetSchemaSql(string backend = "postgres")
        => ReadFile(backend, "schema.sql");

    public static string GetSeedSql(string backend = "postgres")
        => ReadFile(backend, "seed.sql");

    public static string GetGenerateHeerIdSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadFile(backend, Path.Combine(subdir, "generate_heerid.sql"));
    }

    public static string GetGenerateRanjIdSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadFile(backend, Path.Combine(subdir, "generate_ranjid.sql"));
    }

    public static string GetSessionSql(string backend = "postgres")
    {
        var subdir = backend == "mssql" ? "procedures" : "functions";
        return ReadFile(backend, Path.Combine(subdir, "session.sql"));
    }

    private static string NormalizeBackend(string backend)
    {
        if (string.Equals(backend, "postgres", StringComparison.OrdinalIgnoreCase))
            return "postgres";
        if (string.Equals(backend, "mssql", StringComparison.OrdinalIgnoreCase))
            return "mssql";

        throw new ArgumentException("Unsupported backend. Allowed values are 'postgres' or 'mssql'.", nameof(backend));
    }

    private static string ReadFile(string backend, string relativePath)
    {
        var normalizedBackend = NormalizeBackend(backend);
        var path = Path.Combine(SqlBasePath, normalizedBackend, relativePath);
        if (!File.Exists(path))
            throw new FileNotFoundException(
                $"SQL file not found: {path}. Ensure SQL files are available " +
                $"(build with IncludeSql=true or provide sql/ directory).", path);
        return File.ReadAllText(path);
    }
}
