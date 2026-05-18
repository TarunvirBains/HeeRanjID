namespace HeeRanjID;

/// <summary>
/// A UUIDv8-based distributed ID with microsecond precision.
/// Layout: timestamp_micros (90 bits across version/variant gaps) | node_id (16 bits) | sequence (16 bits).
/// </summary>
public readonly struct RanjId : IEquatable<RanjId>, IComparable<RanjId>, IComparable
{
    private static readonly DateTimeOffset Epoch = DateTimeOffset.UnixEpoch;

    private readonly byte[] _bytes;

    private RanjId(byte[] bytes)
    {
        _bytes = bytes;
    }

    internal byte[] GetBytes() => _bytes;

    /// <summary>
    /// Returns a copy of the raw 16-byte big-endian representation.
    /// This is the canonical BINARY(16) / MSSQL-safe wire format; it does NOT
    /// apply the mixed-endian swizzle used by <see cref="ToGuid"/>.
    /// </summary>
    public byte[] ToBytes()
    {
        var copy = new byte[16];
        Array.Copy(_bytes, copy, 16);
        return copy;
    }

    /// <summary>
    /// Constructs a <see cref="RanjId"/> directly from a 16-byte big-endian byte
    /// array without any Guid-endian conversion.  This is the correct path for
    /// SQL Server <c>BINARY(16)</c> columns, which store raw big-endian bytes.
    /// </summary>
    /// <param name="bigEndianBytes">
    /// Exactly 16 bytes in RFC 4122 / big-endian byte order.
    /// </param>
    /// <exception cref="ArgumentException">
    /// Thrown when <paramref name="bigEndianBytes"/> is not exactly 16 bytes.
    /// </exception>
    /// <exception cref="FormatException">
    /// Thrown when the bytes do not represent a valid RanjId (UUIDv8).
    /// </exception>
    public static RanjId FromBytes(byte[] bigEndianBytes)
    {
        if (bigEndianBytes is null || bigEndianBytes.Length != 16)
            throw new ArgumentException("bigEndianBytes must be exactly 16 bytes.", nameof(bigEndianBytes));

        // Validate via native decode to catch non-UUIDv8 inputs early.
        var native = RanjIdBytes.FromArray(bigEndianBytes);
        int rc = NativeMethods.RanjIdDecode(in native, out _, out _, out _);
        if (rc != 0)
            throw new FormatException($"Bytes do not represent a valid RanjId. {NativeMethods.GetLastError()}");

        var copy = new byte[16];
        Array.Copy(bigEndianBytes, copy, 16);
        return new RanjId(copy);
    }

    private RanjIdBytes ToNative() => RanjIdBytes.FromArray(_bytes);

    public ulong TimestampMicros
    {
        get
        {
            var native = ToNative();
            NativeMethods.RanjIdDecode(in native, out ulong ts, out _, out _);
            return ts;
        }
    }

    public ushort NodeId
    {
        get
        {
            var native = ToNative();
            NativeMethods.RanjIdDecode(in native, out _, out ushort node, out _);
            return node;
        }
    }

    public ushort Sequence
    {
        get
        {
            var native = ToNative();
            NativeMethods.RanjIdDecode(in native, out _, out _, out ushort seq);
            return seq;
        }
    }

    public DateTimeOffset Timestamp
    {
        get
        {
            double ms = TimestampMicros / 1000.0;
            return Epoch.AddMilliseconds(ms);
        }
    }

    public static RanjId Parse(string s)
    {
        int rc = NativeMethods.RanjIdFromString(s, out RanjIdBytes result);
        if (rc != 0)
            throw new FormatException($"Invalid RanjId string: {s}. {NativeMethods.GetLastError()}");
        return new RanjId(result.ToArray());
    }

    public static RanjId FromGuid(Guid guid)
    {
        // .NET Guid bytes are in mixed-endian format; UUID bytes are big-endian.
        byte[] guidBytes = guid.ToByteArray();

        // Convert from .NET Guid byte order to RFC4122 (big-endian) byte order
        byte[] uuidBytes = new byte[16];
        uuidBytes[0] = guidBytes[3];
        uuidBytes[1] = guidBytes[2];
        uuidBytes[2] = guidBytes[1];
        uuidBytes[3] = guidBytes[0];
        uuidBytes[4] = guidBytes[5];
        uuidBytes[5] = guidBytes[4];
        uuidBytes[6] = guidBytes[7];
        uuidBytes[7] = guidBytes[6];
        Array.Copy(guidBytes, 8, uuidBytes, 8, 8);

        // Validate via native decode
        var native = RanjIdBytes.FromArray(uuidBytes);
        int rc = NativeMethods.RanjIdDecode(in native, out _, out _, out _);
        if (rc != 0)
            throw new FormatException($"Guid is not a valid RanjId. {NativeMethods.GetLastError()}");

        return new RanjId(uuidBytes);
    }

    public Guid ToGuid()
    {
        // Convert from RFC4122 big-endian bytes to .NET Guid mixed-endian bytes
        byte[] guidBytes = new byte[16];
        guidBytes[3] = _bytes[0];
        guidBytes[2] = _bytes[1];
        guidBytes[1] = _bytes[2];
        guidBytes[0] = _bytes[3];
        guidBytes[5] = _bytes[4];
        guidBytes[4] = _bytes[5];
        guidBytes[7] = _bytes[6];
        guidBytes[6] = _bytes[7];
        Array.Copy(_bytes, 8, guidBytes, 8, 8);

        return new Guid(guidBytes);
    }

    public override string ToString()
    {
        var native = ToNative();
        var buf = new byte[64];
        int n = NativeMethods.RanjIdToString(in native, buf, buf.Length);
        if (n < 0)
            throw new InvalidOperationException(NativeMethods.GetLastError());
        return System.Text.Encoding.UTF8.GetString(buf, 0, n);
    }

    // IEquatable<RanjId>
    public bool Equals(RanjId other)
    {
        if (_bytes is null && other._bytes is null) return true;
        if (_bytes is null || other._bytes is null) return false;
        return _bytes.AsSpan().SequenceEqual(other._bytes);
    }

    public override bool Equals(object? obj) => obj is RanjId other && Equals(other);

    public override int GetHashCode()
    {
        if (_bytes is null) return 0;
        var hash = new HashCode();
        foreach (byte b in _bytes)
            hash.Add(b);
        return hash.ToHashCode();
    }

    // IComparable<RanjId>
    public int CompareTo(RanjId other)
    {
        if (_bytes is null && other._bytes is null) return 0;
        if (_bytes is null) return -1;
        if (other._bytes is null) return 1;
        return _bytes.AsSpan().SequenceCompareTo(other._bytes);
    }

    public int CompareTo(object? obj)
    {
        if (obj is null) return 1;
        if (obj is RanjId other) return CompareTo(other);
        throw new ArgumentException("Object is not a RanjId");
    }

    // Operators
    public static bool operator ==(RanjId left, RanjId right) => left.Equals(right);
    public static bool operator !=(RanjId left, RanjId right) => !left.Equals(right);
}
