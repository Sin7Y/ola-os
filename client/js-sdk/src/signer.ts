import { ethers, sha256, keccak256, toBeArray, SigningKey, BytesLike, hexlify } from "ethers";
import { isValidOlaKey, l2txToTransactionRequest, poseidonHash, rlp_tx, toUint64Array, txRequestToBytes } from "./utils";
import { TransactionType, type L2Tx, type TransactionRequest } from "./types";
import { ENTRYPOINT_ADDRESS } from "./constants";

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
    const hashBytes = poseidonHash(toBeArray(this.publicKey));
    this.address = hexlify(hashBytes);
  }

  getL2Tx(chain_id: number, from: string, nonce: number, calldata: Uint8Array, factory_deps: Array<Uint8Array> | null = null) {
    const fromAddress = Array.from(toUint64Array(from));
    const txRequest: TransactionRequest = {
      nonce,
      from: fromAddress,
      to: ENTRYPOINT_ADDRESS,
      input: calldata,
      type: TransactionType.OlaRawTransaction,
      eip712_meta: { factory_deps, custom_signature: null, paymaster_params: null },
      chain_id,
    };

    // signature in common_data should be 64 bytes.
    const signature = this.signTransactionRequest(txRequest).slice(0, 64);

    const tx: L2Tx = {
      execute: {
        contract_address: ENTRYPOINT_ADDRESS,
        calldata,
        factory_deps,
      },
      common_data: {
        nonce,
        initiator_address: fromAddress,
        signature,
        transaction_type: TransactionType.OlaRawTransaction,
      },
      received_timestamp_ms: Date.now(),
    };

    return tx;
  }

  signMessage(message: string | Uint8Array) {
    if (typeof message === "string" && !message.startsWith("0x")) {
      throw Error("OlaSigner Error: Invalid message. Expected HexString or Uint8Array.");
    }

    const hexMessage = hexlify(message);
    const privKey = new SigningKey(this.privateKey);
    return privKey.sign(hexMessage);
  }

  signTransactionRequest(tx: TransactionRequest) {
    const message = txRequestToBytes(tx);
    const messageHash = poseidonHash(message);
    const signature = this.signMessage(Uint8Array.from(messageHash));
    const sigBytes = new Uint8Array(65);
    sigBytes.set(toBeArray(signature.r), 0);
    sigBytes.set(toBeArray(signature.s), 32);
    sigBytes[64] = signature.v;
    return sigBytes;
  }

  createSignedTransactionRaw(l2tx: L2Tx, chainId: number) {
    const txRequest = l2txToTransactionRequest(l2tx);
    const txRequestSig = this.signTransactionRequest(txRequest);
    const rawTx = rlp_tx(txRequest, txRequestSig, chainId);
    return rawTx;
  }

  createTransaction(chainId: number, nonce: number, calldata: Uint8Array, factory_deps: Array<Uint8Array> | null = null) {
    const l2tx = this.getL2Tx(chainId, this.address, nonce, calldata, factory_deps);
    return this.createSignedTransactionRaw(l2tx, chainId);
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
