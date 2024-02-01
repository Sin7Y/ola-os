import { ethers, hexlify, toBeHex } from "ethers";
import { OlaSigner } from "./signer";
import { OlaProvider } from "./provider";
import { DEFAULT_ACCOUNT_ADDRESS, DEFAULT_CHAIN_ID } from "./constants";
import {
  createEntrypointCalldata,
  encodeAbi,
  decodeAbi,
  toBigintArray,
  toUint64Array,
  toUint8Array,
  capitalize,
} from "./utils";
import { ACCOUNT_ABI } from "./abi";
import { OlaAddress } from "./libs/address";
import { TransactionType, CallResponse } from "./types";

const DEFAULT_RPC_URL = "/";

export class OlaWallet {
  public chainId: number = DEFAULT_CHAIN_ID;

  private constructor(public signer: OlaSigner, public provider: OlaProvider) {}

  get address() {
    return this.signer.address;
  }

  connect(rpcUrl: string, chainId?: number) {
    this.chainId = chainId ?? DEFAULT_CHAIN_ID;
    this.provider = new OlaProvider(rpcUrl, chainId);
  }

  async getNonce() {
    return this.provider.getNonce(this.address);
  }

  /**
   *
   * @param abi
   * @param method
   * @param to DataHexString
   * @param params
   * @param options nonce
   * @returns
   */
  async invoke(
    abi: Array<any>,
    method: string,
    to: string,
    params: Array<any>,
    options?: { nonce: number }
  ) {
    const nonce = options?.nonce ?? (await this.getNonce());

    const bizCalldata = encodeAbi(abi, method, params);
    const entryCalldata = createEntrypointCalldata(this.address, to, bizCalldata);
    const calldata = toUint8Array(entryCalldata);
    const txRaw = this.signer.createTransaction(this.chainId, nonce, calldata, null);
    const txHash = await this.provider.request<string>("ola_sendRawTransaction", {
      tx_bytes: txRaw,
    });
    return txHash;
  }

  async call<T>(abi: Array<any>, method: string, to: string, params: Array<any>) {
    const nonce = await this.getNonce();

    const bizCalldata = encodeAbi(abi, method, params);
    // All fields in CallRequest should be hex string.
    const call_request = {
      from: this.address,
      to: to,
      data: hexlify(toUint8Array(bizCalldata)),
      nonce: toBeHex(nonce),
      transaction_type: toBeHex(TransactionType.OlaRawTransaction),
    };

    const tx = await this.provider.request<string>("ola_callTransaction", {
      call_request,
    });
    const decoded = decodeAbi(abi, method, toUint64Array(tx));
    const outputs = decoded[1][0];
    const outputType = outputs.param.type;
    const outputsValue = outputs.value[capitalize(outputType)];
    return outputsValue as T;
  }

  async setPubKey() {
    return this.invoke(
      ACCOUNT_ABI,
      "setPubkey(fields)",
      hexlify(toUint8Array(DEFAULT_ACCOUNT_ADDRESS)),
      [{ Fields: toBigintArray(this.signer.publicKey) }],
      { nonce: 0 }
    );
  }

  static async fromETHSignature(ethSigner: ethers.Signer, rpcUrl?: string): Promise<OlaWallet> {
    const signer = await OlaSigner.fromETHSignature(ethSigner);
    const provider = new OlaProvider(rpcUrl ?? DEFAULT_RPC_URL);
    return new OlaWallet(signer, provider);
  }
}
