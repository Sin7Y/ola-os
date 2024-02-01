/**
 * capitalize the first letter.
 * @param value
 * @returns
 */
export function capitalize(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

export * from "./crypto";
export * from "./abi";
export * from "./transactioin";
