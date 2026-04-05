using HeeRanjID;
using Microsoft.EntityFrameworkCore.Storage.ValueConversion;

namespace HeeRanjID.EFCore;

/// <summary>
/// EF Core value converter that stores HeerId as a bigint (long) column.
/// </summary>
public class HeerIdValueConverter : ValueConverter<HeerId, long>
{
    public HeerIdValueConverter()
        : base(
            id => id.Value,
            value => new HeerId(value))
    {
    }
}
