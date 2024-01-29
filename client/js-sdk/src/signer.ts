import { ethers, sha256, keccak256, toBeArray, SigningKey, BytesLike } from "ethers";
import { isValidOlaKey } from "./utils";
import init_poseidon, { poseidon_u64_bytes_for_bytes_wrapper } from "./crypto/mini_goldilocks";

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

function pubkeyToUint8Array(pk: BytesLike): Uint8Array {
  if (typeof pk === "string") {
    const encoder = new TextEncoder();
    return encoder.encode(pk);
  } else if (pk instanceof Uint8Array) {
    return pk;
  } else {
    throw new Error("Unsupported data type");
  }
}

export class OlaSigner {
  readonly privateKey: BytesLike;
  readonly publicKey: BytesLike;
  readonly address: string;

  private constructor(privateKey: BytesLike) {
    init_poseidon().then(() => {
        console.log("Init poseidon wasm succeed!");
    });
    this.privateKey = privateKey;
    this.publicKey = computePublicKey(privateKey);
    const pk = pubkeyToUint8Array(this.publicKey);
    // address = Poseidon_hash(pubkey)
    this.address = poseidon_u64_bytes_for_bytes_wrapper(pk);
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
