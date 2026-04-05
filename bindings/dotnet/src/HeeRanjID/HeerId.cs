namespace HeeRanjID;

/// <summary>
/// A snowflake-style 63-bit distributed ID.
/// Layout: timestamp_ms (41 bits) | node_id (9 bits) | sequence (13 bits).
/// </summary>
public readonly struct HeerId : IEquatable<HeerId>, IComparable<HeerId>, IComparable
{
    private static readonly DateTimeOffset Epoch = DateTimeOffset.UnixEpoch;

    public long Value { get; }

    public HeerId(long value)
    {
        if (value < 0)
            throw new ArgumentOutOfRangeException(nameof(value), "HeerId must be non-negative");
        Value = value;
    }

    public ulong TimestampMs
    {
        get
        {
            NativeMethods.HeerIdDecode(Value, out ulong ts, out _, out _);
            return ts;
        }
    }

    public ushort NodeId
    {
        get
        {
            NativeMethods.HeerIdDecode(Value, out _, out ushort node, out _);
            return node;
        }
    }

    public ushort Sequence
    {
        get
        {
            NativeMethods.HeerIdDecode(Value, out _, out _, out ushort seq);
            return seq;
        }
    }

    public DateTimeOffset Timestamp => Epoch.AddMilliseconds(TimestampMs);

    public static HeerId Parse(string s)
    {
        int rc = NativeMethods.HeerIdFromString(s, out long result);
        if (rc != 0)
            throw new FormatException($"Invalid HeerId string: {s}. {NativeMethods.GetLastError()}");
        return new HeerId(result);
    }

    public override string ToString()
    {
        var buf = new byte[32];
        int n = NativeMethods.HeerIdToString(Value, buf, buf.Length);
        if (n < 0)
            throw new InvalidOperationException(NativeMethods.GetLastError());
        return System.Text.Encoding.UTF8.GetString(buf, 0, n);
    }

    // IEquatable<HeerId>
    public bool Equals(HeerId other) => Value == other.Value;
    public override bool Equals(object? obj) => obj is HeerId other && Equals(other);
    public override int GetHashCode() => Value.GetHashCode();

    // IComparable<HeerId>
    public int CompareTo(HeerId other) => Value.CompareTo(other.Value);
    public int CompareTo(object? obj)
    {
        if (obj is null) return 1;
        if (obj is HeerId other) return CompareTo(other);
        throw new ArgumentException("Object is not a HeerId");
    }

    // Operators
    public static bool operator ==(HeerId left, HeerId right) => left.Equals(right);
    public static bool operator !=(HeerId left, HeerId right) => !left.Equals(right);
    public static bool operator <(HeerId left, HeerId right) => left.Value < right.Value;
    public static bool operator >(HeerId left, HeerId right) => left.Value > right.Value;
    public static bool operator <=(HeerId left, HeerId right) => left.Value <= right.Value;
    public static bool operator >=(HeerId left, HeerId right) => left.Value >= right.Value;

    // Conversions
    public static explicit operator long(HeerId id) => id.Value;
    public static explicit operator HeerId(long value) => new(value);
}
