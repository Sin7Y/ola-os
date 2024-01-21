import { BigNumberish, toBigInt } from "ethers";

export function toUint64Array(value: BigNumberish) {
  const bigIntValue = toBigInt(value);
  const mask64 = 2n ** 64n - 1n;
  const parts: bigint[] = [];
  for (let i = 0; i < 4; i++) {
    const part = (bigIntValue >> (BigInt(i) * 64n)) & mask64;
    parts.unshift(part);
  }
  return parts;
}

const maxKeyBoundary = 2n ** 64n - 2n ** 32n + 1n;
export function isValidOlaKey(key: string) {
  return toUint64Array(key).every((item) => item <= maxKeyBoundary);
}
