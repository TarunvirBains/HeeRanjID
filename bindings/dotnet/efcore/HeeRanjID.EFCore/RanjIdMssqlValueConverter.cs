using HeeRanjID;
using Microsoft.EntityFrameworkCore.Storage.ValueConversion;

namespace HeeRanjID.EFCore;

/// <summary>
/// EF Core value converter that stores <see cref="RanjId"/> as a raw 16-byte
/// <c>BINARY(16)</c> column on SQL Server.
/// <para>
/// Unlike <see cref="RanjIdValueConverter"/>, this converter bypasses the
/// <c>uniqueidentifier</c> / <see cref="Guid"/> path entirely.  SQL Server's
/// <c>uniqueidentifier</c> type stores bytes in mixed-endian order (first three
/// groups little-endian, last two big-endian), which scrambles the sort key
/// embedded in the high bits of a RanjId.  Using <c>BINARY(16)</c> preserves
/// the raw big-endian byte layout and keeps DESC index ordering correct.
/// </para>
/// <para>
/// Apply this converter together with <c>HasColumnType("binary(16)")</c> on
/// every <see cref="RanjId"/> property when targeting SQL Server.
/// <see cref="ModelBuilderExtensions.UseHeeRanjIdConverters(Microsoft.EntityFrameworkCore.ModelBuilder, string?)"/>
/// does this automatically when the provider name contains "SqlServer".
/// </para>
/// </summary>
public class RanjIdMssqlValueConverter : ValueConverter<RanjId, byte[]>
{
    /// <summary>Initialises a new instance of <see cref="RanjIdMssqlValueConverter"/>.</summary>
    public RanjIdMssqlValueConverter()
        : base(
            id => id.ToBytes(),
            bytes => RanjId.FromBytes(bytes))
    {
    }
}
