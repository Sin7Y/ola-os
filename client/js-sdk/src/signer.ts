import { ethers, sha256, keccak256, toBeArray, SigningKey, BytesLike } from "ethers";
import { isValidOlaKey } from "./utils";

function computePublicKey(privateKey: BytesLike) {
  return "0x" + SigningKey.computePublicKey(privateKey).slice(4);
}

function privateKeyFromSeed(seed: Uint8Array) {
  let privateKey = sha256(seed);

  let count = 0;
  while (count < 10000) {
    let publicKey = computePublicKey(privateKey);
    if (isValidOlaKey(privateKey) && isValidOlaKey(publicKey)) {
      return privateKey;
    } else {
      privateKey = keccak256(privateKey);
      count++;
    }
  }
}

export class OlaSigner {
  readonly privateKey: BytesLike;
  readonly publicKey: BytesLike;
  readonly address: string;

  constructor(privateKey: BytesLike) {
    this.privateKey = privateKey;
    this.publicKey = computePublicKey(privateKey);
    this.address = keccak256(this.publicKey);
  }

  static async fromETHSignature(ethSigner: ethers.Signer): Promise<OlaSigner> {
    const message = "Access OlaVM.\n" + "\n" + "This account is only for Sepolia testnet.";
    const signature = await ethSigner.signMessage(message);
    const seed = ethers.toBeArray(signature);
    const olaPrivateKey = privateKeyFromSeed(seed);
    if (!olaPrivateKey) throw new Error("Ola SDK: Private Key generate error.");
    return new OlaSigner(olaPrivateKey);
  }
}
