using HeeRanjID;
using Xunit;

namespace HeeRanjID.Tests;

public class HeerIdTests
{
    [Fact]
    public void Constructor_RejectsNegative()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new HeerId(-1));
    }

    [Fact]
    public void Constructor_AcceptsZero()
    {
        var id = new HeerId(0);
        Assert.Equal(0L, id.Value);
    }

    [Fact]
    public void Decode_ReturnsCorrectParts()
    {
        // timestamp=1234567, node=42, seq=777
        long raw = ((long)1234567 << 22) | ((long)42 << 13) | 777;
        var id = new HeerId(raw);

        Assert.Equal(1234567UL, id.TimestampMs);
        Assert.Equal((ushort)42, id.NodeId);
        Assert.Equal((ushort)777, id.Sequence);
    }

    [Fact]
    public void Parse_RoundTrips()
    {
        long raw = ((long)1000 << 22) | ((long)5 << 13) | 42;
        var id = new HeerId(raw);
        string s = id.ToString();
        var parsed = HeerId.Parse(s);

        Assert.Equal(id, parsed);
    }

    [Fact]
    public void Parse_RejectsGarbage()
    {
        Assert.Throws<FormatException>(() => HeerId.Parse("not_a_number"));
    }

    [Fact]
    public void Parse_RejectsNegative()
    {
        Assert.Throws<FormatException>(() => HeerId.Parse("-1"));
    }

    [Fact]
    public void Equality_Works()
    {
        var a = new HeerId(42);
        var b = new HeerId(42);
        var c = new HeerId(43);

        Assert.Equal(a, b);
        Assert.NotEqual(a, c);
        Assert.True(a == b);
        Assert.True(a != c);
    }

    [Fact]
    public void Comparison_Works()
    {
        var a = new HeerId(10);
        var b = new HeerId(20);

        Assert.True(a < b);
        Assert.True(b > a);
        Assert.True(a.CompareTo(b) < 0);
    }

    [Fact]
    public void Timestamp_ReturnsDateTimeOffset()
    {
        // timestamp_ms = 1000 means 1 second after Unix epoch
        long raw = (long)1000 << 22;
        var id = new HeerId(raw);
        var expected = DateTimeOffset.UnixEpoch.AddMilliseconds(1000);

        Assert.Equal(expected, id.Timestamp);
    }

    [Fact]
    public void ExplicitCast_RoundTrips()
    {
        var id = new HeerId(12345);
        long value = (long)id;
        var back = (HeerId)value;

        Assert.Equal(id, back);
    }
}
