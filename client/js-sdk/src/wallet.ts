import { ethers } from "ethers";
import { OlaSigner } from "./signer";
import { OlaProvider } from "./provider";

const DEFAULT_RPC_URL = "/";

export class OlaWallet {
  private constructor(public signer: OlaSigner, public provider: OlaProvider) {}

  get address() {
    return this.signer.address;
  }

  connect(rpcUrl: string) {
    this.provider = new OlaProvider(rpcUrl);
  }

  async getNonce() {
    return this.provider.getNonce(this.address);
  }

  async invoke(abi: Array<any>, method: string, from: string, to: string, params: Array<any>) {
    const nonce = await this.getNonce();
    console.log("nonce", nonce);

    const tx = await this.provider.request("ola_sendRawTransaction", { tx_bytes: "" });
    console.log("tx", tx);
  }

  async call(abi: Array<any>, method: string, from: string, to: string, params: Array<any>) {
    const tx = await this.provider.request("ola_sendRawTransaction", { tx_bytes: "" });
    console.log("tx", tx);
  }

  changePubKey() {
    return this.invoke([], "", "", "", []);
  }

  static async fromETHSignature(ethSigner: ethers.Signer, rpcUrl?: string): Promise<OlaWallet> {
    const signer = await OlaSigner.fromETHSignature(ethSigner);
    const provider = new OlaProvider(rpcUrl ?? DEFAULT_RPC_URL);
    return new OlaWallet(signer, provider);
  }
}
