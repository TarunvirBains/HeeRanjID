#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "../heeranjid.h"

static int tests_passed = 0;
static int tests_failed = 0;

#define TEST(name) do { printf("  test %s ... ", #name); } while(0)
#define PASS() do { printf("ok\n"); tests_passed++; } while(0)
#define FAIL(msg) do { printf("FAILED: %s\n", msg); tests_failed++; } while(0)

/* ── HeerId tests ────────────────────────────────────────────────── */

void test_heer_id_decode(void) {
    TEST(heer_id_decode);

    /* Construct an ID with known parts: timestamp=1234567, node=42, seq=777 */
    /* From Rust: HeerId::new(1234567, 42, 777) */
    /* Layout: timestamp(41) | node(9) | seq(13) */
    HeerIdT id = ((int64_t)1234567 << 22) | ((int64_t)42 << 13) | 777;

    uint64_t ts = 0;
    uint16_t node = 0;
    uint16_t seq = 0;

    int rc = heer_id_decode(id, &ts, &node, &seq);
    if (rc != 0) { FAIL("decode returned error"); return; }
    if (ts != 1234567) { FAIL("timestamp mismatch"); return; }
    if (node != 42) { FAIL("node_id mismatch"); return; }
    if (seq != 777) { FAIL("sequence mismatch"); return; }

    PASS();
}

void test_heer_id_rejects_negative(void) {
    TEST(heer_id_rejects_negative);

    uint64_t ts;
    uint16_t node, seq;
    int rc = heer_id_decode(-1, &ts, &node, &seq);
    if (rc != -1) { FAIL("should reject negative"); return; }

    char errbuf[256];
    int n = heer_last_error(errbuf, sizeof(errbuf));
    if (n <= 0) { FAIL("no error message"); return; }

    PASS();
}

void test_heer_id_string_roundtrip(void) {
    TEST(heer_id_string_roundtrip);

    HeerIdT id = ((int64_t)1000 << 22) | ((int64_t)5 << 13) | 42;

    char buf[64];
    int n = heer_id_to_string(id, buf, sizeof(buf));
    if (n < 0) { FAIL("to_string failed"); return; }

    HeerIdT parsed;
    int rc = heer_id_from_string(buf, &parsed);
    if (rc != 0) { FAIL("from_string failed"); return; }
    if (parsed != id) { FAIL("roundtrip mismatch"); return; }

    PASS();
}

void test_heer_id_from_string_rejects_garbage(void) {
    TEST(heer_id_from_string_rejects_garbage);

    HeerIdT out;
    int rc = heer_id_from_string("not_a_number", &out);
    if (rc != -1) { FAIL("should reject garbage"); return; }

    PASS();
}

void test_heer_id_zero(void) {
    TEST(heer_id_zero);

    uint64_t ts;
    uint16_t node, seq;
    int rc = heer_id_decode(0, &ts, &node, &seq);
    if (rc != 0) { FAIL("decode failed"); return; }
    if (ts != 0 || node != 0 || seq != 0) { FAIL("zero parts mismatch"); return; }

    PASS();
}

/* ── RanjId tests ────────────────────────────────────────────────── */

void test_ranj_id_string_roundtrip(void) {
    TEST(ranj_id_string_roundtrip);

    /* Create a valid UUIDv7 string by going through the FFI.
     * We'll construct one by encoding known parts via the Rust side.
     * First let's use a known good UUID string from Rust tests. */

    /* We need a valid UUIDv7. Let's construct bytes manually.
     * RanjId::new(1_000_000, 100, 200) produces a valid UUIDv7.
     * But we don't have a create function in FFI, so let's work with
     * the string representation.
     *
     * Actually let's just test with heer_id first, then parse a known UUID.
     * From Rust: RanjId::new(1_000_000, 100, 200)
     * timestamp_high = 1_000_000 >> 42 = 0
     * timestamp_mid  = (1_000_000 >> 30) & 0xFFF = 0
     * timestamp_low  = 1_000_000 & 0x3FFFFFFF = 1000000
     * raw = (0 << 80) | (7 << 76) | (0 << 64) | (2 << 62) | (1000000 << 32) | (100 << 16) | 200
     * = 0x0000_7000_8000_0000_0F42_4000_6400_C8 ... let's just get the string from Rust.
     */

    /* Since we can't construct from parts in C, let's test the string parsing
     * with a UUID that we know is valid UUIDv7 format. We'll create one by
     * constructing the raw bytes. */

    /* UUIDv7 layout (big-endian):
     * bits 0-47: timestamp_high
     * bits 48-51: version (0x7)
     * bits 52-63: timestamp_mid
     * bits 64-65: variant (0b10)
     * bits 66-95: timestamp_low
     * bits 96-111: node_id
     * bits 112-127: sequence
     */

    /* timestamp_micros=1000000, node_id=100, sequence=200 */
    /* timestamp_high = 1000000 >> 42 = 0 */
    /* timestamp_mid  = (1000000 >> 30) & 0xFFF = 0 */
    /* timestamp_low  = 1000000 & 0x3FFFFFFF = 1000000 = 0xF4240 */
    /* raw bytes (big-endian u128):
     * byte 0-5:  0x000000000000  (timestamp_high)
     * byte 6:    0x70            (version=7, timestamp_mid high)
     * byte 7:    0x00            (timestamp_mid low)
     * byte 8:    0x80            (variant=10, timestamp_low high bits)
     * byte 9:    0x00            (timestamp_low continued)
     * byte 10:   0x0F            (timestamp_low continued)
     * byte 11:   0x42            (timestamp_low continued)
     * byte 12:   0x40            (timestamp_low low + node_id high)
     * Wait, let me recalculate properly. */

    /* raw u128 = (0 << 80) | (7 << 76) | (0 << 64) | (2 << 62) | (1000000 << 32) | (100 << 16) | 200 */
    /* = (7 << 76) | (2 << 62) | (1000000 << 32) | (100 << 16) | 200 */
    /* = 0x00000000000070008000000F4240006400C8 */
    /* Actually u128 = 0x0000_0000_0000_7000_8000_000F_4240_00640_0C8 */
    /* Hmm, let me just be precise:
     * 7 << 76      = 0x0000_0000_0000_7000_0000_0000_0000_0000
     * 2 << 62      = 0x0000_0000_0000_0000_8000_0000_0000_0000
     * 1000000 << 32= 0x0000_0000_0000_0000_0000_000F_4240_0000_0000
     * Wait: 1000000 = 0xF4240
     * 0xF4240 << 32 = 0x0000_0000_0000_0000_0000_000F_4240_0000_0000
     * Hmm that's more than 128 bits. Let me think again.
     *
     * u128 layout: 128 bits total
     * Bit 127 is MSB.
     * 7 << 76: sets bits 76-79 to 0111
     * 2 << 62: sets bits 62-63 to 10
     * 1000000 << 32: 1000000 = 0xF4240, shifted left 32
     * 100 << 16: node_id
     * 200: sequence
     */

    /* Let me just compute this with actual values.
     * 7u128 << 76  = a large number. In hex bytes (big-endian u128, 16 bytes):
     * byte[0..6] = 0x00 0x00 0x00 0x00 0x00 0x00
     * byte[6] has bits 76-79 in it. Byte 6 is bits 72-79.
     *   76-79 = 0111 in bits 76-79, rest 0 = 0x70
     * byte[7] = 0x00
     *
     * 2u128 << 62:
     * Byte 8 is bits 56-63. bit 62-63 = 10 = 0x80 in that byte.
     * byte[8] = 0x80
     *
     * 1000000u128 << 32 = 0xF4240_0000_0000
     * This occupies bits 32 to ~51 (1000000 is 20 bits).
     * Byte 9 is bits 48-55: bits 48-51 have upper 4 bits of 0xF4240 >> 16 = 0x0F
     *   But wait: 1000000 << 32 needs bits starting at bit 32.
     *   bit 32-39 -> byte 11: 0x00
     *   Actually 1000000 = 0x000F4240
     *   0x000F4240 << 32:
     *     bits 32-63: 0x000F4240
     *     byte 8 (bits 56-63): 0x00 -> but byte 8 already has variant
     *     Actually we OR them. Let me just compute the full u128 numerically.
     */

    /* I'll use a simpler approach: construct from the known UUID string
     * that Rust would produce. Let me just hardcode the test. */

    /* Actually, the easiest approach is: construct bytes for a known valid
     * RanjId, convert to string, parse back, compare bytes. */

    struct RanjIdT rid;
    /* Build raw u128 manually. We need version=7 (bits 76-79) and variant=2 (bits 62-63) */
    /* Simplest valid RanjId: all zeros except version and variant */
    memset(&rid, 0, sizeof(rid));
    /* UUID bytes are big-endian. Byte indices:
     * byte[6] high nibble = version -> set to 0x70
     * byte[8] high 2 bits = variant -> set to 0x80
     */
    rid.bytes[6] = 0x70;
    rid.bytes[8] = 0x80;

    char buf[64];
    int n = ranj_id_to_string(&rid, buf, sizeof(buf));
    if (n < 0) { FAIL("to_string failed"); return; }

    struct RanjIdT parsed;
    int rc = ranj_id_from_string(buf, &parsed);
    if (rc != 0) { FAIL("from_string failed"); return; }
    if (memcmp(&rid.bytes, &parsed.bytes, 16) != 0) { FAIL("roundtrip mismatch"); return; }

    PASS();
}

void test_ranj_id_decode(void) {
    TEST(ranj_id_decode);

    /* Use the same minimal valid RanjId */
    struct RanjIdT rid;
    memset(&rid, 0, sizeof(rid));
    rid.bytes[6] = 0x70;
    rid.bytes[8] = 0x80;
    /* Set node_id=0x0064 (100) in bytes 12-13 and sequence=0x00C8 (200) in bytes 14-15 */
    rid.bytes[12] = 0x00; rid.bytes[13] = 0x64; /* node_id = 100 */
    rid.bytes[14] = 0x00; rid.bytes[15] = 0xC8; /* sequence = 200 */

    uint64_t ts;
    uint16_t node, seq;
    int rc = ranj_id_decode(&rid, &ts, &node, &seq);
    if (rc != 0) { FAIL("decode failed"); return; }
    if (node != 100) { FAIL("node_id mismatch"); return; }
    if (seq != 200) { FAIL("sequence mismatch"); return; }

    PASS();
}

void test_ranj_id_rejects_bad_version(void) {
    TEST(ranj_id_rejects_bad_version);

    struct RanjIdT rid;
    memset(&rid, 0, sizeof(rid));
    /* version = 4 instead of 7 */
    rid.bytes[6] = 0x40;
    rid.bytes[8] = 0x80;

    uint64_t ts;
    uint16_t node, seq;
    int rc = ranj_id_decode(&rid, &ts, &node, &seq);
    if (rc != -1) { FAIL("should reject bad version"); return; }

    PASS();
}

void test_ranj_id_from_string_rejects_garbage(void) {
    TEST(ranj_id_from_string_rejects_garbage);

    struct RanjIdT out;
    int rc = ranj_id_from_string("not-a-uuid", &out);
    if (rc != -1) { FAIL("should reject garbage"); return; }

    PASS();
}

int main(void) {
    printf("running HeerId FFI tests:\n");
    test_heer_id_decode();
    test_heer_id_rejects_negative();
    test_heer_id_string_roundtrip();
    test_heer_id_from_string_rejects_garbage();
    test_heer_id_zero();

    printf("\nrunning RanjId FFI tests:\n");
    test_ranj_id_string_roundtrip();
    test_ranj_id_decode();
    test_ranj_id_rejects_bad_version();
    test_ranj_id_from_string_rejects_garbage();

    printf("\n%d passed, %d failed\n", tests_passed, tests_failed);
    return tests_failed > 0 ? 1 : 0;
}
