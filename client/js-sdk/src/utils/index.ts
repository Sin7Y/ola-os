import { BigNumberish, toBigInt, toBeArray } from "ethers";

/**
 * HexString -> BigUint64Array
 * @param value
 * @returns
 */
export function toUint64Array(value: Uint8Array | BigNumberish) {
  if (value instanceof Uint8Array) {
    if (value.length % 8 != 0) {
      throw new Error("Input must have exactly 8 elements.");
    }

    let chunkLenth = Math.floor(value.length / 8);
    const out = new BigUint64Array(chunkLenth);
    for (let i = 0; i < chunkLenth; i++) {
      const buff = Buffer.from(value.slice(8 * i, 8 * (i + 1)));
      out[i] = buff.readBigUInt64BE(0);
    }
    return out;
  }

  const bigIntValue = toBigInt(value);
  const mask64 = 2n ** 64n - 1n;
  const parts: bigint[] = [];
  for (let i = 0; i < 4; i++) {
    const part = (bigIntValue >> (BigInt(i) * 64n)) & mask64;
    // console.log("part", part);
    parts.unshift(part);
  }
  // console.log("....", parts);

  // let index = 0;
  // while (true) {
  //   const part = (bigIntValue >> (BigInt(index) * 64n)) & mask64;
  //   console.log("part", part);
  //   if (!part) break;
  //   parts.unshift(part);
  //   index++;
  // }
  // console.log("....", parts);

  return BigUint64Array.from(parts);
}

const maxKeyBoundary = 2n ** 64n - 2n ** 32n + 1n;
export function isValidOlaKey(key: string) {
  return toUint64Array(key).every((item) => item <= maxKeyBoundary);
}

/**
 * BigUint64Array -> Uint8Array
 * @param arr
 * @returns
 */
export function toUint8Array(value: BigUint64Array | bigint[] | bigint) {
  const length = typeof value === "bigint" ? 1 : value.length;
  const array = typeof value === "bigint" ? [value] : value;
  const buffer = new ArrayBuffer(length * 8); // 8 bytes per Uint64
  const dataView = new DataView(buffer);

  for (let i = 0; i < length; i++) {
    // Set values in big-endian order
    dataView.setBigUint64(i * 8, array[i], false);
  }
  return new Uint8Array(buffer);
}

// Convert a hex string to a byte array
export function hexToBytes(hex: string) {
    let bytes = [];
    for (let c = 0; c < hex.length; c += 2)
        bytes.push(parseInt(hex.substr(c, 2), 16));
    return bytes;
}

export * from "./abi";
export * from "./transactioin";
