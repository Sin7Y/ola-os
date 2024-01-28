import { BigNumberish, BytesLike, ethers } from "ethers";
import init, { encode_input_from_js } from "./crypto/ola_lang_abi";
// import { U256, H256 } from "./types";
import { OlaSigner } from "./signer";
import { Sign } from "crypto";

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
    nonce?: number;
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
    // let r = l2tx.common_data.signature.slice(0,32).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    // let s = l2tx.common_data.signature.slice(32,64).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    let r = uint8ArrayToUint64Arrays(l2tx.common_data.signature.slice(0,32));
    let s = uint8ArrayToUint64Arrays(l2tx.common_data.signature.slice(32,64));
    let tx_req: TransactionRequest = {
        nonce: l2tx.common_data.nonce,
        from: l2tx.common_data.initiator_address,
        input: l2tx.execute.calldata,
        v: 27,
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

function createSignedTransactionRaw(l2tx: L2Tx, signer: OlaSigner) {
    let chain_id = 1027;
    let tx_req =l2txToTransactionRequest(l2tx);
    let signature = signTransactionRequest(signer, tx_req);
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
function signTransactionRequest(signer: OlaSigner, transaction_request: TransactionRequest): Uint8Array {
    // TODO: TransactionRequest convert to Uint8Array and then use Secp256k1 sign it.
    return new Uint8Array(65);
}

function econstructL2Tx(signer: OlaSigner, chain_id: number, from: Address, nonce: number, calldata: Uint8Array, factory_deps: null | Array<Uint8Array>): L2Tx {
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
    let signature = signTransactionRequest(signer, req);

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
    const entrypointAbiJson = require("./Entrypoint_abi.json");
    const method = "system_entrance";
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
    await init();
    const abiJson = JSON.parse(abi);
    console.log("abiJson: ", abiJson);
    console.log("method: ", method);
    console.log("params: ", params);
    let biz_calldata = encode_input_from_js(abiJson, method, params);
    let entrypoint_calldata = createEntrypointCalldata(from, to, biz_calldata, codes);
    let calldata = uint64ArrayToBytes(entrypoint_calldata);
    return calldata;
}

export function createTransaction(signer: OlaSigner, chain_id: number, from: Address, nonce: number, calldata: Uint8Array, factory_deps: Uint8Array[] | null) {
    let l2tx = econstructL2Tx(signer, chain_id, from, nonce, calldata, factory_deps);
    let raw_tx = createSignedTransactionRaw(l2tx, signer);
    return raw_tx;
}

export function parseTx(strTx: string): L2Tx {
    const parsedTx = JSON.parse(strTx) as L2Tx;
    return parsedTx;
}