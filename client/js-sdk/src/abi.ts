import { toUtf8Bytes } from "ethers";
import init, { encode_input_from_js } from "./crypto/ola_lang_abi";

export async function encodeAbi(abi: any[], method: string, params: Record<string, any>[]) {
  await init();
  const abiJson = JSON.stringify(abi);
  const abiBuffer = toUtf8Bytes(abiJson);
  const result = encode_input_from_js(abiBuffer, method, params);
  return result;
}

export function decodeAbi() {}
