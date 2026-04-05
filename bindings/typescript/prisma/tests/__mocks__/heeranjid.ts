/**
 * Lightweight stub of the native `heeranjid` NAPI module.
 * Used by vitest so shape/SQL tests can run without a compiled binary.
 */

export class HeerId {
  constructor(private readonly value: bigint) {}

  static fromBigInt(value: bigint): HeerId {
    return new HeerId(value);
  }

  toBigInt(): bigint {
    return this.value;
  }
}

export class RanjId {
  constructor(private readonly value: string) {}

  static fromString(value: string): RanjId {
    return new RanjId(value);
  }

  toString(): string {
    return this.value;
  }
}
