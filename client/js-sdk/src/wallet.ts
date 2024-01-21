import { ethers } from "ethers";
import { OlaSigner } from "./signer";

export class OlaWallet {
  constructor(public signer: OlaSigner) {}

  get address() {
    return this.signer.address;
  }

  public invoke(abi: Array<any>, method: string, from: string, to: string, params: Array<any>) {}

  public call(abi: Array<any>, method: string, from: string, to: string, params: Array<any>) {}

  public changePubKey() {
    this.invoke([], "", "", "", []);
  }

  static async fromETHSignature(ethSigner: ethers.Signer): Promise<OlaWallet> {
    const signer = await OlaSigner.fromETHSignature(ethSigner);

    return new OlaWallet(signer);
  }
}
