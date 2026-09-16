// Byte helpers for the value encoding: little-endian scalars, u32 length
// then payload. What kernel/interface/src/codec.rs does.

export class Writer {
  private parts: number[] = [];

  u8(value: number): void {
    this.parts.push(value & 0xff);
  }

  u16(value: number): void {
    this.u8(value);
    this.u8(value >>> 8);
  }

  u32(value: number): void {
    this.u16(value & 0xffff);
    this.u16(value >>> 16);
  }

  u64(value: bigint): void {
    this.u32(Number(value & 0xffffffffn));
    this.u32(Number((value >> 32n) & 0xffffffffn));
  }

  bytes(bytes: Uint8Array): void {
    this.u32(bytes.length);
    for (const b of bytes) this.parts.push(b);
  }

  str(text: string): void {
    this.bytes(new TextEncoder().encode(text));
  }

  done(): Uint8Array {
    return Uint8Array.from(this.parts);
  }
}

/** Reads one value after another; throws once the bytes run out. */
export class Reader {
  private at = 0;
  private view: DataView;

  constructor(private input: Uint8Array) {
    this.view = new DataView(input.buffer, input.byteOffset, input.byteLength);
  }

  done(): boolean {
    return this.at === this.input.length;
  }

  private take(len: number): number {
    if (this.at + len > this.input.length) throw new RangeError('short');
    const at = this.at;
    this.at += len;
    return at;
  }

  u8(): number {
    return this.view.getUint8(this.take(1));
  }

  u16(): number {
    return this.view.getUint16(this.take(2), true);
  }

  u32(): number {
    return this.view.getUint32(this.take(4), true);
  }

  u64(): bigint {
    return this.view.getBigUint64(this.take(8), true);
  }

  i64(): bigint {
    return this.view.getBigInt64(this.take(8), true);
  }

  f64(): number {
    return this.view.getFloat64(this.take(8), true);
  }

  bool(): boolean {
    const b = this.u8();
    if (b > 1) throw new RangeError('not a bool');
    return b === 1;
  }

  bytes(): Uint8Array {
    const len = this.u32();
    const at = this.take(len);
    return this.input.slice(at, at + len);
  }

  str(): string {
    return new TextDecoder('utf-8', { fatal: true }).decode(this.bytes());
  }
}
