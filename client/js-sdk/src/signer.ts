import { ethers, sha256, keccak256, toBeArray, SigningKey, BytesLike, hexlify } from "ethers";
import { isValidOlaKey } from "./utils";
import { poseidon_u64_bytes_for_bytes_wrapper } from "@sin7y/ola-crypto";

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
  readonly publicKey: BytesLike;
  readonly address: string;

  private constructor(readonly privateKey: BytesLike) {
    this.publicKey = computePublicKey(privateKey);
    const hashBytes = poseidon_u64_bytes_for_bytes_wrapper(toBeArray(this.publicKey)) as number[];
    this.address = hexlify(Uint8Array.from(hashBytes));
  }

  static async fromETHSignature(ethSigner: ethers.Signer): Promise<OlaSigner> {
    const message = "Access OlaVM.\n" + "\n" + "This account is only for Sepolia testnet.";
    const signature = await ethSigner.signMessage(message);
    const seed = toBeArray(signature);
    const olaPrivateKey = privateKeyFromSeed(seed);
    if (!olaPrivateKey) throw new Error("Ola SDK: Private Key generate error.");
    return new OlaSigner(olaPrivateKey);
  }
}
