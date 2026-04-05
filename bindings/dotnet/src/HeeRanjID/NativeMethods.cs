using System.Runtime.InteropServices;
using System.Text;

namespace HeeRanjID;

[StructLayout(LayoutKind.Sequential, Size = 16)]
internal unsafe struct RanjIdBytes
{
    public fixed byte Bytes[16];

    public byte[] ToArray()
    {
        var arr = new byte[16];
        for (int i = 0; i < 16; i++)
            arr[i] = Bytes[i];
        return arr;
    }

    public static RanjIdBytes FromArray(byte[] bytes)
    {
        var result = new RanjIdBytes();
        for (int i = 0; i < 16; i++)
            result.Bytes[i] = bytes[i];
        return result;
    }
}

internal static partial class NativeMethods
{
    private const string LibName = "heeranjid_ffi";

    [LibraryImport(LibName, EntryPoint = "heer_last_error")]
    private static unsafe partial int HeerLastErrorNative(byte* buf, int bufLen);

    [LibraryImport(LibName, EntryPoint = "heer_id_decode")]
    internal static partial int HeerIdDecode(
        long id, out ulong timestampMs, out ushort nodeId, out ushort sequence);

    [LibraryImport(LibName, EntryPoint = "heer_id_to_string")]
    private static unsafe partial int HeerIdToStringNative(
        long id, byte* buf, int bufLen);

    [LibraryImport(LibName, EntryPoint = "heer_id_from_string",
        StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int HeerIdFromString(string s, out long result);

    [LibraryImport(LibName, EntryPoint = "ranj_id_decode")]
    private static unsafe partial int RanjIdDecodeNative(
        RanjIdBytes* id, out ulong timestampUs, out ushort nodeId, out ushort sequence);

    [LibraryImport(LibName, EntryPoint = "ranj_id_to_string")]
    private static unsafe partial int RanjIdToStringNative(
        RanjIdBytes* id, byte* buf, int bufLen);

    [LibraryImport(LibName, EntryPoint = "ranj_id_from_string",
        StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial int RanjIdFromStringNative(string s, RanjIdBytes* result);

    internal static unsafe int RanjIdDecode(in RanjIdBytes id, out ulong timestampUs, out ushort nodeId, out ushort sequence)
    {
        fixed (RanjIdBytes* ptr = &id)
        {
            return RanjIdDecodeNative(ptr, out timestampUs, out nodeId, out sequence);
        }
    }

    internal static unsafe int RanjIdToString(in RanjIdBytes id, byte[] buf, int bufLen)
    {
        fixed (RanjIdBytes* idPtr = &id)
        fixed (byte* bufPtr = buf)
        {
            return RanjIdToStringNative(idPtr, bufPtr, bufLen);
        }
    }

    internal static unsafe int RanjIdFromString(string s, out RanjIdBytes result)
    {
        RanjIdBytes tmp = default;
        int rc = RanjIdFromStringNative(s, &tmp);
        result = tmp;
        return rc;
    }

    internal static unsafe int HeerIdToString(long id, byte[] buf, int bufLen)
    {
        fixed (byte* ptr = buf)
        {
            return HeerIdToStringNative(id, ptr, bufLen);
        }
    }

    internal static unsafe string GetLastError()
    {
        var buf = new byte[512];
        int n;
        fixed (byte* ptr = buf)
        {
            n = HeerLastErrorNative(ptr, buf.Length);
        }
        if (n <= 0) return string.Empty;
        return Encoding.UTF8.GetString(buf, 0, n);
    }
}
