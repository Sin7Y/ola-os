import { BigNumberish, toBigInt, toBeArray, zeroPadBytes, BytesLike } from "ethers";
import { poseidon_u64_bytes_for_bytes_wrapper } from "@sin7y/ola-crypto";

/**
 * BigNumberish / Uint8Array -> BigUint64Array
 * @param value
 * @returns
 */
export function toUint64Array(value: Uint8Array | BigNumberish) {
  let bytes = value instanceof Uint8Array ? value : toBeArray(value);
  if (bytes.length % 8 !== 0) {
    const remain = 8 - (bytes.length % 8);
    const padding = new Uint8Array(remain).fill(0);
    bytes = new Uint8Array([...padding, ...bytes]);
  }
  const chunkLength = Math.ceil(bytes.length / 8);
  const result = new BigUint64Array(chunkLength);
  for (let i = 0; i < chunkLength; i++) {
    const value = toBigInt(bytes.slice(i * 8, 8 * (i + 1)));
    result[i] = value;
  }
  return result;
}

/**
 * convert HexString address -> bigint[]
 * @param address
 * @returns
 */
export function toBigintArray(address: BytesLike) {
  return Array.from(toUint64Array(address));
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
  for (let c = 0; c < hex.length; c += 2) bytes.push(parseInt(hex.substr(c, 2), 16));
  return bytes;
}

export function poseidonHash(data: Uint8Array) {
  return Uint8Array.from(poseidon_u64_bytes_for_bytes_wrapper(data));
}

export * from "./abi";
export * from "./transactioin";
