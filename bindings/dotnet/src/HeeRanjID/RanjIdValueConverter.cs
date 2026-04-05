using Microsoft.EntityFrameworkCore.Storage.ValueConversion;

namespace HeeRanjID;

/// <summary>
/// EF Core value converter that stores RanjId as a Guid (uuid) column.
/// </summary>
public class RanjIdValueConverter : ValueConverter<RanjId, Guid>
{
    public RanjIdValueConverter()
        : base(
            id => id.ToGuid(),
            value => RanjId.FromGuid(value))
    {
    }
}
