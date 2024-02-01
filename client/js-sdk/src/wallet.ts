import { ethers, hexlify } from "ethers";
import { OlaSigner } from "./signer";
import { OlaProvider } from "./provider";
import { DEFAULT_ACCOUNT_ADDRESS, DEFAULT_CHAIN_ID } from "./constants";
import {
  createEntrypointCalldata,
  encodeAbi,
  toBigintArray,
  toUint64Array,
  toUint8Array,
} from "./utils";
import { ACCOUNT_ABI } from "./abi";
import { OlaAddress } from "./libs/address";

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

  async invoke(abi: Array<any>, method: string, to: string, params: Array<any>) {
    const nonce = await this.getNonce();
    console.log("nonce", nonce);

    const bizCalldata = await encodeAbi(abi, method, params);
    const entryCalldata = await createEntrypointCalldata(this.address, to, bizCalldata);
    const calldata = toUint8Array(entryCalldata);
    const txRaw = this.signer.createTransaction(this.chainId, nonce, calldata, null);
    const txHash = await this.provider.request("ola_sendRawTransaction", { tx_bytes: txRaw });
    console.log("tx", txHash);
  }

  // @todo: call function
  async call(abi: Array<any>, method: string, to: string, params: Array<any>) {
    const tx = await this.provider.request("ola_callTransaction", { tx_bytes: "" });
    console.log("tx", tx);
  }

  changePubKey() {
    return this.invoke(
      ACCOUNT_ABI,
      "setPubkey(fields)",
      hexlify(toUint8Array(DEFAULT_ACCOUNT_ADDRESS)),
      [{ Fields: OlaAddress.toBigintArray(this.signer.publicKey) }]
    );
  }

  static async fromETHSignature(ethSigner: ethers.Signer, rpcUrl?: string): Promise<OlaWallet> {
    const signer = await OlaSigner.fromETHSignature(ethSigner);
    const provider = new OlaProvider(rpcUrl ?? DEFAULT_RPC_URL);
    return new OlaWallet(signer, provider);
  }
}
