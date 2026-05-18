using HeeRanjID;
using Xunit;

namespace HeeRanjID.Tests;

public class RanjIdTests
{
    // A valid UUIDv8 string (version=8, variant=RFC4122)
    // This corresponds to RanjId with timestamp=0, node=100, seq=200
    private const string ValidUuidString = "00000000-0000-8000-8000-0000006400c8";

    // Big-endian byte representation of ValidUuidString:
    //   time_low(4) | time_mid(2) | time_hi_ver(2) | clock_seq(2) | node(6)
    // Bytes: 00 00 00 00  00 00  80 00  80 00  00 00 00 64 00 C8
    private static readonly byte[] ValidUuidBytes =
    {
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
        0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xC8
    };

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

    // -----------------------------------------------------------------------
    // MSSQL BINARY(16) / byte-order-preservation tests
    // -----------------------------------------------------------------------

    /// <summary>
    /// FromBytes must preserve the exact big-endian bytes supplied.
    /// This is the invariant required for BINARY(16) SQL Server storage:
    /// bytes written to the column must come back bit-for-bit identical.
    /// </summary>
    [Fact]
    public void FromBytes_PreservesBigEndianByteOrder()
    {
        var id = RanjId.FromBytes(ValidUuidBytes);
        var roundTripped = id.ToBytes();

        Assert.Equal(ValidUuidBytes, roundTripped);
    }

    /// <summary>
    /// Parsing the same value from its string representation and constructing
    /// via FromBytes must yield equal RanjIds (same identity, different path).
    /// </summary>
    [Fact]
    public void FromBytes_EqualsToParsedString()
    {
        var fromBytes = RanjId.FromBytes(ValidUuidBytes);
        var fromString = RanjId.Parse(ValidUuidString);

        Assert.Equal(fromString, fromBytes);
    }

    /// <summary>
    /// Demonstrates that the pre-fix MSSQL path — new Guid(bytes) followed by
    /// RanjId.FromGuid — silently scrambles the byte layout.
    /// <para>
    /// new Guid(byte[]) treats bytes 0-3 as a little-endian int32, bytes 4-5 as
    /// a little-endian int16, and bytes 6-7 as a little-endian int16.
    /// RanjId.FromGuid then re-swaps those groups to recover RFC 4122 order.
    /// The net effect is identity only when byte groups 0-3, 4-5, and 6-7 are
    /// palindromes — which they are not for a real RanjId with non-zero
    /// timestamp bits in the high byte positions.
    /// </para>
    /// <para>
    /// This test uses a hand-crafted byte array with distinct non-zero values in
    /// the first seven bytes, so the swap corruption is visible.
    /// </para>
    /// </summary>
    [Fact]
    public void GuidPath_ScramblesByteOrder_UnlikeFromBytes()
    {
        // A valid UUIDv8 with distinct non-zero bytes in the first 8 positions
        // so that the little-endian swaps performed by new Guid(byte[]) are
        // observable.  Layout:
        //   bytes 0-3 : time_low  = 0x01 02 03 04
        //   bytes 4-5 : time_mid  = 0x05 06
        //   bytes 6-7 : time_hi + version = 0x80 07  (high nibble of byte 6 = 8 → UUIDv8)
        //   bytes 8-9 : clock_seq + variant = 0x80 09 (top 2 bits of byte 8 = 10 → RFC4122)
        //   bytes 10-15: node = 0x0A 0B 0C 0D 0E 0F
        var distinguishableBytes = new byte[]
        {
            0x01, 0x02, 0x03, 0x04,   // time_low  (bytes 0-3)
            0x05, 0x06,               // time_mid  (bytes 4-5)
            0x80, 0x07,               // time_hi + version=8 (bytes 6-7)
            0x80, 0x09,               // clock_seq + variant (bytes 8-9)
            0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F  // node (bytes 10-15)
        };

        // Via the correct FromBytes path: bytes are preserved as-is.
        var viaFromBytes = RanjId.FromBytes(distinguishableBytes);
        Assert.Equal(distinguishableBytes, viaFromBytes.ToBytes());

        // Simulate what the pre-fix code did: new Guid(bytes) + FromGuid.
        //
        // new Guid(byte[]) + guid.ToByteArray() is identity — the bytes come
        // back unchanged.  The corruption happens because FromGuid is designed
        // to decode a .NET Guid whose byte layout is ALREADY mixed-endian
        // (i.e., produced by ToGuid()).  When fed raw big-endian BINARY(16)
        // bytes instead, FromGuid treats them as if they were a Guid and applies
        // the RFC4122 de-swizzle — reversing groups [0-3], [4-5], [6-7] one
        // more time rather than cancelling the previous swap.
        //
        // Trace for distinguishableBytes:
        //   guidBytes = new Guid(b).ToByteArray() = b (identity)
        //   uuidBytes[0] = guidBytes[3] = 0x04   ← was b[0] = 0x01
        //   uuidBytes[1] = guidBytes[2] = 0x03   ← was b[1] = 0x02
        //   uuidBytes[2] = guidBytes[1] = 0x02
        //   uuidBytes[3] = guidBytes[0] = 0x01
        //   uuidBytes[4] = guidBytes[5] = 0x06   ← was b[4] = 0x05
        //   uuidBytes[5] = guidBytes[4] = 0x05
        //   uuidBytes[6] = guidBytes[7] = 0x07   ← was b[6] = 0x80 (version 8)
        //   uuidBytes[7] = guidBytes[6] = 0x80
        //   Result bytes[6] = 0x07, high nibble = 0x0 → version 0, not UUIDv8
        //
        // The native decoder rejects version 0 → FormatException.
        // This is the direct proof that the old code was broken: a valid
        // BINARY(16) RanjId read from the database became invalid/corrupted.
        Assert.Throws<FormatException>(() => RanjId.FromGuid(new Guid(distinguishableBytes)));
    }

    /// <summary>
    /// FromBytes with a null or wrong-length array must throw ArgumentException,
    /// not silently truncate or produce a corrupt RanjId.
    /// </summary>
    [Fact]
    public void FromBytes_RejectsNullArray()
    {
        Assert.Throws<ArgumentException>(() => RanjId.FromBytes(null!));
    }

    [Fact]
    public void FromBytes_RejectsWrongLength()
    {
        Assert.Throws<ArgumentException>(() => RanjId.FromBytes(new byte[15]));
        Assert.Throws<ArgumentException>(() => RanjId.FromBytes(new byte[17]));
        Assert.Throws<ArgumentException>(() => RanjId.FromBytes(Array.Empty<byte>()));
    }

    /// <summary>
    /// FromBytes validates the UUIDv8 invariant via the native decoder.
    /// A 16-byte buffer that doesn't encode a valid UUIDv8 must be rejected.
    /// </summary>
    [Fact]
    public void FromBytes_RejectsInvalidVersion()
    {
        // Build a byte array where the version nibble is 4 (UUIDv4), not 8.
        // Byte 6 carries the version in its high nibble: 0x40 = version 4.
        var v4Bytes = (byte[])ValidUuidBytes.Clone();
        v4Bytes[6] = 0x40;

        Assert.Throws<FormatException>(() => RanjId.FromBytes(v4Bytes));
    }

    /// <summary>
    /// ToBytes must return a defensive copy — mutating the returned array
    /// must not corrupt the internal state of the RanjId.
    /// </summary>
    [Fact]
    public void ToBytes_ReturnsDefensiveCopy()
    {
        var id = RanjId.FromBytes(ValidUuidBytes);
        var bytes = id.ToBytes();
        bytes[0] = 0xFF; // mutate the copy

        // The original RanjId must be unchanged
        Assert.Equal(ValidUuidBytes, id.ToBytes());
    }

    /// <summary>
    /// FromBytes must take a defensive copy of the caller-supplied array so that
    /// mutations to the input after the call cannot corrupt the stored RanjId state.
    /// </summary>
    [Fact]
    public void FromBytes_TakesDefensiveCopyOfInput()
    {
        // Use a mutable clone so we can freely mutate it after the call.
        var input = (byte[])ValidUuidBytes.Clone();

        var id = RanjId.FromBytes(input);

        // Snapshot the RanjId's bytes before we tamper with the input.
        var snapshot = id.ToBytes();

        // Corrupt the caller-owned array after FromBytes has returned.
        input[0] = (byte)(input[0] ^ 0xFF);

        // The RanjId's internal state must be unchanged — it held a copy, not
        // a reference to the input array.
        Assert.Equal(snapshot, id.ToBytes());
    }

    /// <summary>
    /// RanjIdMssqlValueConverter round-trip: RanjId -> byte[] -> RanjId must
    /// preserve identity.  This exercises the converter pair used in the EF Core
    /// MSSQL model configuration path.
    /// </summary>
    [Fact]
    public void RanjIdMssqlValueConverter_RoundTrips()
    {
        var converter = new HeeRanjID.EFCore.RanjIdMssqlValueConverter();

        var original = RanjId.FromBytes(ValidUuidBytes);

        // Simulate EF Core write path: RanjId -> byte[]
        var toProvider = converter.ConvertToProviderExpression.Compile();
        var bytes = toProvider(original);

        // Simulate EF Core read path: byte[] -> RanjId
        var fromProvider = converter.ConvertFromProviderExpression.Compile();
        var roundTripped = fromProvider(bytes);

        Assert.Equal(original, roundTripped);
        Assert.Equal(ValidUuidBytes, bytes);
    }
}
