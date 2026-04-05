using HeeRanjID;
using Xunit;

namespace HeeRanjID.Tests;

public class SqlHelperTests
{
    [Fact]
    public void GetInstallSql_ReturnsNonEmpty()
    {
        string sql = SqlHelper.GetInstallSql();
        Assert.False(string.IsNullOrWhiteSpace(sql));
        Assert.Contains("CREATE", sql, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void GetSchemaSql_ReturnsNonEmpty()
    {
        string sql = SqlHelper.GetSchemaSql();
        Assert.False(string.IsNullOrWhiteSpace(sql));
    }

    [Fact]
    public void GetSeedSql_ReturnsNonEmpty()
    {
        string sql = SqlHelper.GetSeedSql();
        Assert.False(string.IsNullOrWhiteSpace(sql));
    }

    [Fact]
    public void GetResourceNames_ContainsSqlFiles()
    {
        string[] names = SqlHelper.GetResourceNames();
        Assert.NotEmpty(names);
        Assert.All(names, n => Assert.EndsWith(".sql", n));
    }

    [Fact]
    public void GetGenerateHeerIdSql_ReturnsNonEmpty()
    {
        string sql = SqlHelper.GetGenerateHeerIdSql();
        Assert.False(string.IsNullOrWhiteSpace(sql));
    }

    [Fact]
    public void GetGenerateRanjIdSql_ReturnsNonEmpty()
    {
        string sql = SqlHelper.GetGenerateRanjIdSql();
        Assert.False(string.IsNullOrWhiteSpace(sql));
    }
}
