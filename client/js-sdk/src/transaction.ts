import { BigNumberish, BytesLike, ethers } from "ethers";
import init_abi, { encode_input_from_js } from "./crypto/ola_lang_abi";
import init_poseidon, { poseidon_u64_bytes_for_bytes_wrapper } from "./crypto/mini_goldilocks";
// import { U256, H256 } from "./types";
import { OlaSigner } from "./signer";
import { secp256k1 } from "@noble/curves/secp256k1";

export type Address = BigUint64Array;

// 32 bytes
export type H256 = Uint8Array;

// 4 u64s
export type U256 = BigUint64Array;

// 65 bytes
export type Signature = Uint8Array;

const ENTRYPOINT_ADDRESS: Address = new BigUint64Array([0x0n, 0x0n, 0x0n, 0x8001n]);

interface Execute {
    contract_address: Address;
    calldata: Uint8Array;
    factory_deps?: Array<Uint8Array> | null;
}

interface InputData {
    hash: H256;
    data: Uint8Array;
}

export enum TransactionType {
    EIP712Transaction = 0,
    EIP1559Transaction = 1,
    OlaRawTransaction = 2,
    PriorityOpTransaction = 3,
    ProtocolUpgradeTransaction = 4,
}

interface L2TxCommonData {
    nonce: number;
    initiator_address: Address;
    signature: Uint8Array;
    transaction_type: TransactionType;
    input?: InputData | null;
}

export interface L2Tx {
    execute: Execute;
    common_data: L2TxCommonData;
    received_timestamp_ms: number;
}

export type PaymasterParams = {
    paymaster: Address;
    paymaster_input: Uint8Array;
};

export type Eip712Meta = {
    factory_deps?: null | Array<Uint8Array>;
    custom_signature?: null | Uint8Array;
    paymaster_params?: null | PaymasterParams;
};

export interface TransactionRequest {
    nonce: number;
    from?: null | Address;
    to?: null | Address;
    input: Uint8Array;
    v?: null | number,
    r?: null | U256,
    s?: null | U256,
    raw?: null | Uint8Array,
    type?: null | number;
    eip712_meta?: null | Eip712Meta;
    chain_id?: null | number;
};

// Function to convert an array of Uint64 values to bytes
function uint64ArrayToBytes(uint64Array: BigUint64Array) {
    const buffer = new ArrayBuffer(uint64Array.length * 8); // 8 bytes per Uint64
    const dataView = new DataView(buffer);

    for (let i = 0; i < uint64Array.length; i++) {
        // Set values in big-endian order
        dataView.setBigUint64(i * 8, uint64Array[i], false);
    }
    return new Uint8Array(buffer);
}

// Function to convert an array of Bigint values to bytes
function bigintArrayToBytes(uint64Array: bigint[]) {
    const buffer = new ArrayBuffer(uint64Array.length * 8); // 8 bytes per Uint64
    const dataView = new DataView(buffer);

    for (let i = 0; i < uint64Array.length; i++) {
        // Set values in big-endian order
        dataView.setBigUint64(i * 8, uint64Array[i], false);
    }
    return new Uint8Array(buffer);
}

function bigintToUint8Array(b: bigint) {
    const buffer = new ArrayBuffer(8);
    const dataView = new DataView(buffer);
    dataView.setBigUint64(0, b, false);
    return new Uint8Array(buffer);
}

// Function to convert an array of Uint8 values to Uint64 array.
function uint8ArrayToUint64Arrays(uint8s: Uint8Array) {
    if (uint8s.length % 8 != 0) {
        throw new Error("Input must have exactly 8 elements.");
    }

    let chunk_len = Math.floor(uint8s.length / 8);
    const out = new BigUint64Array(chunk_len);
    for (let i = 0; i < chunk_len; i++) {
        const buff = Buffer.from(uint8s.slice(8*i, 8*(i+1)));
        out[i] = buff.readBigUInt64BE(0);
    }
    return out;
}

// Function to convert address to Uint8Array
function addressToUint8Array(addr: Address) {
    const u8s = uint64ArrayToBytes(addr);
    return u8s;
}

export async function sendTransaction(tx: string): Promise<string> {
    // let tx_hash = await provider.send_raw_transaction(tx)?;
    // return tx_hash;
    return "";
}

function l2txToTransactionRequest(l2tx: L2Tx) {
    // TODO: chainid should be extract from common_data
    let chain_id = 1027;
    let tx_type = l2tx.common_data.transaction_type;
    let r = uint8ArrayToUint64Arrays(l2tx.common_data.signature.slice(0,32));
    let s = uint8ArrayToUint64Arrays(l2tx.common_data.signature.slice(32,64));
    let v = l2tx.common_data.signature[64];
    let tx_req: TransactionRequest = {
        nonce: l2tx.common_data.nonce,
        from: l2tx.common_data.initiator_address,
        input: l2tx.execute.calldata,
        v: v,
        r: r,
        s: s,
        type: tx_type,
        chain_id: chain_id,
    };
    switch (tx_type) {
        case TransactionType.EIP1559Transaction:
            break;
        case TransactionType.EIP712Transaction:
        case TransactionType.OlaRawTransaction:
            tx_req.eip712_meta = {
                factory_deps: l2tx.execute.factory_deps,
                custom_signature: l2tx.common_data.signature,
                paymaster_params: null
            };
        default:
            throw new Error("Invalid transaction type: " + tx_type);
            break;
    }
    return tx_req;
}

async function createSignedTransactionRaw(l2tx: L2Tx, signer: OlaSigner) {
    let chain_id = 1027;
    let tx_req = l2txToTransactionRequest(l2tx);
    let signature = await signTransactionRequest(signer, tx_req);
    let signed_bytes = rlp_tx(tx_req, signature, chain_id);

    return signed_bytes;
}

function rlp_tx(tx: TransactionRequest, signature: Uint8Array, chain_id: number) {
    if (signature.length != 65) {
        throw Error("Signature length must be 65");
    }
    if (tx.type != TransactionType.EIP1559Transaction && tx.type != TransactionType.EIP712Transaction && tx.type != TransactionType.OlaRawTransaction) {
        throw Error("Unknown transaction type");
    }

    const fields: any[] = [];
    if (tx.type == TransactionType.EIP1559Transaction) {
        fields.push(ethers.toBeArray(chain_id));
    }
    if (tx.nonce != null) {
        fields.push(ethers.toBeArray(tx.nonce));
    }
    if (tx.to != null) {
        fields.push(addressToUint8Array(tx.to));
    } 
    fields.push(tx.input);

    // Signature
    fields.push(ethers.toBeArray(signature[0]));
    fields.push(signature.slice(0, 32));
    fields.push(signature.slice(32, 64));

    // EIP712 || OLA
    if (tx.type == TransactionType.EIP712Transaction || tx.type == TransactionType.OlaRawTransaction) {
        fields.push(ethers.toBeArray(chain_id));
        if (tx.from != null) {
            fields.push(addressToUint8Array(tx.from));
        }

        if (tx.eip712_meta != null) {
            fields.push((tx.eip712_meta.factory_deps ?? []).map((dep) => ethers.hexlify(dep)));
            if (tx.eip712_meta.custom_signature && ethers.getBytes(tx.eip712_meta.custom_signature).length == 0) {
                throw new Error("Empty signatures are not supported");
            }
            fields.push(tx.eip712_meta.custom_signature || "0x");

            if (tx.eip712_meta.paymaster_params) {
                fields.push([
                    tx.eip712_meta.paymaster_params.paymaster,
                    ethers.hexlify(tx.eip712_meta.paymaster_params.paymaster_input),
                ]);
            } else {
                fields.push([]);
            }
        }
    }

    return ethers.concat([new Uint8Array([TransactionType.OlaRawTransaction]), ethers.encodeRlp(fields)]);
}

// TODO: use ECDSA sign the TransactionRequest.
async function signTransactionRequest(signer: OlaSigner, tx: TransactionRequest): Promise<Uint8Array> {
    await init_poseidon();
    // TODO: TransactionRequest convert to Uint8Array and then use Secp256k1 sign it.
    let message = transactionRequestToBytes(tx);
    let msg_hash = poseidon_u64_bytes_for_bytes_wrapper(message);
    const signature = secp256k1.sign(msg_hash, signer.privateKey, {
        lowS: true
    });
    const r = bigintToUint8Array(signature.r);
    const s = bigintToUint8Array(signature.s);
    const v = signature.recovery ? 0x1c: 0x1b;
    let sig = new Uint8Array(65);
    sig.set(r, 0);
    sig.set(s, 32);
    sig[64] = v;
    return sig;
}

// Convert TransactionRequest to Uint8Array
function transactionRequestToBytes(tx: TransactionRequest) {
    if (tx.eip712_meta == null) {
        throw new Error("We can sign transaction only with meta");
    }
    if (tx.eip712_meta.paymaster_params != null && tx.eip712_meta.paymaster_params.paymaster_input.length % 8 != 0) {
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

    let input = uint8ArrayToUint64Arrays(tx.input);
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
        paymaster_input = uint8ArrayToUint64Arrays(tx.eip712_meta.paymaster_params.paymaster_input);
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
    return bigintArrayToBytes(data);
}

async function econstructL2Tx(signer: OlaSigner, chain_id: number, from: Address, nonce: number, calldata: Uint8Array, factory_deps: null | Array<Uint8Array>): Promise<L2Tx> {
    let req: TransactionRequest = {
        nonce: nonce,
        from: from,
        to: ENTRYPOINT_ADDRESS,
        input: calldata,
        type: TransactionType.OlaRawTransaction,
        eip712_meta: {
            factory_deps: factory_deps,
        },
        chain_id: chain_id
    };
    let signature = await signTransactionRequest(signer, req);

    let tx: L2Tx = {
        execute: {
            contract_address: ENTRYPOINT_ADDRESS,
            calldata: calldata,
            factory_deps: factory_deps
        },
        common_data: {
            nonce: nonce,
            initiator_address: from,
            signature: signature,
            transaction_type: TransactionType.OlaRawTransaction
        },
        received_timestamp_ms: Date.now(),
    };

    return tx;
}

function createEntrypointCalldata(from: Address, to: Address, calldata: any, codes: number[] | null) {
    const entrypointAbiJson = require("./ssystem_contract/Entrypoint_abi.json");
    const method = "system_entrance((address,address,fields,fields),bool)";
    const params = [{
            Tuple: [
                ["address", { Address: from }],
                ["address", { Address: to }],
                ["fields", { Fields: calldata }],
                ["fields", { Fields: codes }]
        ]},
        { Bool: false }
    ];

    let data = encode_input_from_js(entrypointAbiJson, method, params);
    return data;
}

export async function createCalldata(from: Address, to: Address, abi: string, method: string, params: any, codes: number[] | null) {
    await init_abi();
    const abiJson = JSON.parse(abi);
    let biz_calldata = encode_input_from_js(abiJson, method, params);
    let entrypoint_calldata = createEntrypointCalldata(from, to, biz_calldata, codes);
    let calldata = uint64ArrayToBytes(entrypoint_calldata);
    return calldata;
}

export async function createTransaction(signer: OlaSigner, chain_id: number, from: Address, nonce: number, calldata: Uint8Array, factory_deps: Uint8Array[] | null) {
    let l2tx = await econstructL2Tx(signer, chain_id, from, nonce, calldata, factory_deps);
    let raw_tx = await createSignedTransactionRaw(l2tx, signer);
    return raw_tx;
}

export function parseTx(strTx: string): L2Tx {
    const parsedTx = JSON.parse(strTx) as L2Tx;
    return parsedTx;
}