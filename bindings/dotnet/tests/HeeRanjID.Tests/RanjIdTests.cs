using HeeRanjID;
using Xunit;

namespace HeeRanjID.Tests;

public class RanjIdTests
{
    // A valid UUIDv8 string (version=8, variant=RFC4122)
    // This corresponds to RanjId with timestamp=0, node=100, seq=200
    private const string ValidUuidString = "00000000-0000-8000-8000-0000006400c8";

    [Fact]
    public void Parse_ValidUuid_Succeeds()
    {
        var id = RanjId.Parse(ValidUuidString);
        Assert.Equal((ushort)100, id.NodeId);
        Assert.Equal((ushort)200, id.Sequence);
    }

    [Fact]
    public void Parse_RoundTrips()
    {
        var id = RanjId.Parse(ValidUuidString);
        string s = id.ToString();
        var parsed = RanjId.Parse(s);

        Assert.Equal(id, parsed);
    }

    [Fact]
    public void Parse_RejectsGarbage()
    {
        Assert.Throws<FormatException>(() => RanjId.Parse("not-a-uuid"));
    }

    [Fact]
    public void Parse_RejectsWrongVersion()
    {
        // UUIDv4 instead of v8
        Assert.Throws<FormatException>(() =>
            RanjId.Parse("12345678-1234-4000-8000-123456789abc"));
    }

    [Fact]
    public void FromGuid_ToGuid_RoundTrips()
    {
        var id = RanjId.Parse(ValidUuidString);
        Guid guid = id.ToGuid();
        var back = RanjId.FromGuid(guid);

        Assert.Equal(id, back);
    }

    [Fact]
    public void Equality_Works()
    {
        var a = RanjId.Parse(ValidUuidString);
        var b = RanjId.Parse(ValidUuidString);

        Assert.Equal(a, b);
        Assert.True(a == b);
    }

    [Fact]
    public void Decode_ReturnsCorrectParts()
    {
        var id = RanjId.Parse(ValidUuidString);
        Assert.Equal(0UL, id.TimestampMicros);
        Assert.Equal((ushort)100, id.NodeId);
        Assert.Equal((ushort)200, id.Sequence);
    }

    [Fact]
    public void FromGuid_RejectsInvalidVersion()
    {
        // Create a Guid with version 4, not 8
        var badGuid = Guid.Parse("12345678-1234-4000-8000-123456789abc");
        Assert.Throws<FormatException>(() => RanjId.FromGuid(badGuid));
    }
}
