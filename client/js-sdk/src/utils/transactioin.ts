import { ENTRYPOINT_ADDRESS } from "../constants";
import { OlaSigner } from "../signer";
import { encodeAbi } from "./abi";
import { toUint64Array, toUint8Array, poseidonHash } from "./crypto";
import { L2Tx, TransactionRequest, TransactionType } from "../types";
import { ethers, getBytes, hexlify, toBeArray } from "ethers";
import { ENTRYPOINT_ABI } from "../abi/entrypoint";
import { OlaAddress } from "../libs/address";

export function createEntrypointCalldata(
  from: string,
  to: string,
  calldata: BigUint64Array,
  codes: number[] = []
) {
  const method = "system_entrance((address,address,fields,fields),bool)";
  const params = [
    {
      Tuple: [
        ["address", { Address: Array.from(OlaAddress.toBigintArray(from)) }],
        ["address", { Address: Array.from(OlaAddress.toBigintArray(to)) }],
        ["fields", { Fields: calldata }],
        ["fields", { Fields: codes }],
      ],
    },
    { Bool: false },
  ];

  let data = encodeAbi(ENTRYPOINT_ABI, method, params);
  return data;
}

export function txRequestToBytes(tx: TransactionRequest) {
  if (tx.eip712_meta == null) {
    throw new Error("We can sign transaction only with meta");
  }
  if (
    tx.eip712_meta.paymaster_params != null &&
    tx.eip712_meta.paymaster_params.paymaster_input.length % 8 != 0
  ) {
    throw new Error("Paymaster input must be 8-byte aligned");
  }
  if (tx.input.length % 8 != 0) {
    throw new Error("Transaction data must be 8-byte aligned");
  }
  if (tx.chain_id == null) {
    throw new Error("Chain id must be set when perform sign");
  }
  if (tx.from == undefined || tx.from == null) {
    throw new Error("We can only sign transactions with known sender");
  }

  let input = toUint64Array(tx.input);
  let pos_biz_calldata_start = 8;
  let biz_calldata_len = Number(input[pos_biz_calldata_start]);
  let pos_biz_calldata_end = pos_biz_calldata_start + biz_calldata_len + 1;
  let biz_input = input.slice(pos_biz_calldata_start, pos_biz_calldata_end);
  let biz_addr = input.slice(4, 8);

  let paymaster_address = null;
  let paymaster_input_len = null;
  let paymaster_input = null;
  if (tx.eip712_meta.paymaster_params != null) {
    paymaster_address = tx.eip712_meta.paymaster_params.paymaster;
    paymaster_input_len = Math.floor(tx.eip712_meta.paymaster_params.paymaster_input.length / 8);
    paymaster_input = toUint64Array(tx.eip712_meta.paymaster_params.paymaster_input);
  }

  let data: bigint[] = [];
  data.push(BigInt(tx.chain_id));
  data.push(BigInt(tx.type ?? TransactionType.OlaRawTransaction));
  data.push(BigInt(tx.nonce));
  data.push(...tx.from);
  data.push(...biz_addr);
  data.push(...biz_input);
  if (paymaster_address != null) {
    data.push(...paymaster_address);
  }
  if (paymaster_input_len != null) {
    data.push(BigInt(paymaster_input_len));
  }
  if (paymaster_input != null) {
    data.push(...paymaster_input);
  }
  return toUint8Array(data);
}

async function signTransactionRequest(signer: OlaSigner, tx: TransactionRequest) {
  const message = txRequestToBytes(tx);
  const messageHash = poseidonHash(message);
  const signature = signer.signMessage(Uint8Array.from(messageHash));
  const sigBytes = new Uint8Array(65);
  sigBytes.set(toBeArray(signature.r), 0);
  sigBytes.set(toBeArray(signature.s), 32);
  sigBytes[64] = signature.v;
  return sigBytes;
}

export async function getL2Tx(
  signer: OlaSigner,
  chain_id: number,
  from: string,
  nonce: number,
  calldata: Uint8Array,
  factory_deps: Array<Uint8Array> | null = null
) {
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
  const signature = signer.signTransactionRequest(txRequest).slice(0, 64);

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

export function l2txToTransactionRequest(l2tx: L2Tx) {
  let chain_id = 1027;
  let tx_type = l2tx.common_data.transaction_type;
  let r = toUint64Array(l2tx.common_data.signature.slice(0, 32));
  let s = toUint64Array(l2tx.common_data.signature.slice(32, 64));
  let v = l2tx.common_data.signature[64];
  let txRequest: TransactionRequest = {
    nonce: l2tx.common_data.nonce,
    from: l2tx.common_data.initiator_address,
    to: l2tx.execute.contract_address,
    input: l2tx.execute.calldata,
    v,
    r,
    s,
    type: tx_type,
    chain_id: chain_id,
  };
  switch (tx_type) {
    case TransactionType.EIP1559Transaction:
      break;
    case TransactionType.EIP712Transaction:
    case TransactionType.OlaRawTransaction:
      txRequest.eip712_meta = {
        factory_deps: l2tx.execute.factory_deps,
        custom_signature: l2tx.common_data.signature,
        paymaster_params: null,
      };
      break;
    default:
      throw new Error("Invalid transaction type: " + tx_type);
  }
  return txRequest;
}
/**
 * encode TransactionRequest
 * @param tx
 * @param signature
 * @param chain_id
 * @returns RLP-encoded HexDataString
 */
export function rlp_tx(tx: TransactionRequest, signature: Uint8Array, chain_id: number) {
  if (signature.length != 65) {
    throw Error("Signature length must be 65");
  }
  if (
    tx.type != TransactionType.EIP1559Transaction &&
    tx.type != TransactionType.EIP712Transaction &&
    tx.type != TransactionType.OlaRawTransaction
  ) {
    throw Error("Unknown transaction type");
  }

  const fields: any[] = [];
  if (tx.type == TransactionType.EIP1559Transaction) {
    fields.push(toBeArray(chain_id));
  }
  if (tx.nonce != null) {
    fields.push(toBeArray(tx.nonce));
  }
  if (tx.to != null) {
    fields.push(toUint8Array(tx.to));
  }
  fields.push(tx.input);

  // Signature
  fields.push(toBeArray(signature[0]));
  fields.push(signature.slice(0, 32));
  fields.push(signature.slice(32, 64));

  // EIP712 || OLA
  if (
    tx.type == TransactionType.EIP712Transaction ||
    tx.type == TransactionType.OlaRawTransaction
  ) {
    fields.push(toBeArray(chain_id));
    if (tx.from != null) {
      fields.push(toUint8Array(tx.from));
    }

    if (tx.eip712_meta != null) {
      fields.push((tx.eip712_meta.factory_deps ?? []).map((dep) => hexlify(dep)));
      if (
        tx.eip712_meta.custom_signature &&
        getBytes(tx.eip712_meta.custom_signature).length == 0
      ) {
        throw new Error("Empty signatures are not supported");
      }
      fields.push(tx.eip712_meta.custom_signature || "0x");

      if (tx.eip712_meta.paymaster_params) {
        fields.push([
          tx.eip712_meta.paymaster_params.paymaster,
          hexlify(tx.eip712_meta.paymaster_params.paymaster_input),
        ]);
      } else {
        fields.push([]);
      }
    }
  }

  return ethers.concat([
    new Uint8Array([TransactionType.OlaRawTransaction]),
    ethers.encodeRlp(fields),
  ]);
}
async function createSignedTransactionRaw(l2tx: L2Tx, signer: OlaSigner, chainId: number) {
  const txRequest = l2txToTransactionRequest(l2tx);

  const signature = await signTransactionRequest(signer, txRequest);
  const signed_bytes = rlp_tx(txRequest, signature, chainId);

  return signed_bytes;
}

export async function createTransaction(
  signer: OlaSigner,
  chainId: number,
  from: string,
  nonce: number,
  calldata: Uint8Array,
  factory_deps: Array<Uint8Array> | null = null
) {
  const l2tx = await getL2Tx(signer, chainId, from, nonce, calldata, factory_deps);
  const raw_tx = await createSignedTransactionRaw(l2tx, signer, chainId);
  return raw_tx;
}
