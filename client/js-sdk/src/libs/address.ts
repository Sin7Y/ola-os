import type { BigNumberish } from "ethers";
import { toUint64Array } from "../utils";

export class OlaAddress {
  static toBigintArray(value: Uint8Array | BigNumberish) {
    const bytes = toUint64Array(value);
    const padCount = 4 - bytes.length;
    const padArray = padCount > 0 ? new BigUint64Array(padCount).fill(0n) : null;
    const result = padArray ? new BigUint64Array([...padArray, ...bytes]) : bytes.slice(0, 4);
    return Array.from(result);
  }
}
