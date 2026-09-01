const PNG_SIGNATURE = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

const crc32 = (bytes: Uint8Array): number => {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
};

const latin1 = (value: string): Uint8Array => {
  const bytes = new Uint8Array(value.length);
  for (let i = 0; i < value.length; i += 1) {
    const code = value.charCodeAt(i);
    if (code > 255) {
      throw new Error("PNG tEXt is Latin-1");
    }
    bytes[i] = code;
  }
  return bytes;
};

const readU32 = (bytes: Uint8Array, offset: number): number =>
  ((bytes[offset] << 24) |
    (bytes[offset + 1] << 16) |
    (bytes[offset + 2] << 8) |
    bytes[offset + 3]) >>>
  0;

const writeU32 = (bytes: Uint8Array, offset: number, value: number) => {
  bytes[offset] = (value >>> 24) & 0xff;
  bytes[offset + 1] = (value >>> 16) & 0xff;
  bytes[offset + 2] = (value >>> 8) & 0xff;
  bytes[offset + 3] = value & 0xff;
};

const assertPng = (bytes: Uint8Array) => {
  if (
    bytes.length < PNG_SIGNATURE.length ||
    PNG_SIGNATURE.some((byte, index) => bytes[index] !== byte)
  ) {
    throw new Error("Not a PNG");
  }
};

const findIend = (bytes: Uint8Array): number => {
  let offset = PNG_SIGNATURE.length;
  while (offset + 12 <= bytes.length) {
    const length = readU32(bytes, offset);
    const type = String.fromCharCode(
      bytes[offset + 4],
      bytes[offset + 5],
      bytes[offset + 6],
      bytes[offset + 7],
    );
    if (type === "IEND") {
      return offset;
    }
    offset += 12 + length;
  }
  throw new Error("PNG IEND missing");
};

/** Insert a tEXt chunk before IEND. Keyword is 1–79 Latin-1 chars. */
export const injectPngText = (png: Uint8Array, keyword: string, text: string): Uint8Array => {
  assertPng(png);
  if (keyword.length < 1 || keyword.length > 79 || keyword.includes("\0")) {
    throw new Error("Invalid PNG tEXt keyword");
  }
  const data = new Uint8Array(keyword.length + 1 + text.length);
  data.set(latin1(keyword), 0);
  data[keyword.length] = 0;
  data.set(latin1(text), keyword.length + 1);

  const typeAndData = new Uint8Array(4 + data.length);
  typeAndData.set([0x74, 0x45, 0x58, 0x74]); // tEXt
  typeAndData.set(data, 4);
  const crc = crc32(typeAndData);

  const chunk = new Uint8Array(12 + data.length);
  writeU32(chunk, 0, data.length);
  chunk.set(typeAndData, 4);
  writeU32(chunk, 8 + data.length, crc);

  const iend = findIend(png);
  const out = new Uint8Array(png.length + chunk.length);
  out.set(png.subarray(0, iend), 0);
  out.set(chunk, iend);
  out.set(png.subarray(iend), iend + chunk.length);
  return out;
};

export const readPngText = (png: Uint8Array, keyword: string): string | null => {
  assertPng(png);
  let offset = PNG_SIGNATURE.length;
  while (offset + 12 <= png.length) {
    const length = readU32(png, offset);
    const type = String.fromCharCode(
      png[offset + 4],
      png[offset + 5],
      png[offset + 6],
      png[offset + 7],
    );
    if (type === "tEXt") {
      const data = png.subarray(offset + 8, offset + 8 + length);
      const split = data.indexOf(0);
      if (split > 0) {
        const key = String.fromCharCode(...data.subarray(0, split));
        if (key === keyword) {
          return String.fromCharCode(...data.subarray(split + 1));
        }
      }
    }
    if (type === "IEND") {
      return null;
    }
    offset += 12 + length;
  }
  return null;
};
