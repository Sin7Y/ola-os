import { BigNumberish, BytesLike, ethers } from "ethers";
import { encode_input_from_js } from "./crypto/ola_lang_abi"
import { U256, H256 } from "./types";
import { OlaSigner } from "./signer";

export type AddressLike = string | Promise<string>;

export type Address = [number, number, number, number];
export type Signature = string;

const ENTRYPOINT_ADDRESS: Address = [0, 0, 0, 0x8001];

interface Execute {
    contract_address: Address;
    calldata: string;
    factory_deps?: number[][] | null;
}

interface InputData {
    hash: H256;
    data: number[];
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
    signature: number[];
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
    paymasterInput: BytesLike;
};

export type Eip712Meta = {
    factoryDeps?: BytesLike[];
    customSignature?: BytesLike;
    paymasterParams?: PaymasterParams;
};

export interface TransactionRequest {
    type?: null | number;
    to?: null | AddressLike;
    from?: null | AddressLike;
    nonce?: number;
    input?: null | string;
    chainId?: null | BigNumberish;
    customData?: null | Eip712Meta;
};

export async function sendTransaction(tx: string): Promise<string> {
    // let tx_hash = await provider.send_raw_transaction(tx)?;
    // return tx_hash;
    return "";
}

export function createTransactionRaw(l2tx: L2Tx, signer: OlaSigner) {
    // TODO: chainid should be extract from common_data
    let chain_id = 1027;
    let tx_type = l2tx.common_data.transaction_type;
    let r = l2tx.common_data.signature.slice(0,32).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    let s = l2tx.common_data.signature.slice(32,64).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    let tx_req: TransactionRequest = {
        nonce: l2tx.common_data.nonce,
        from: l2tx.common_data.initiator_address,
        input: l2tx.execute.calldata,
        v: 27,
        r: U256.from(r),
        s: U256.from(s),
        transaction_type: tx_type,
        chain_id: chain_id,
    };
    switch (tx_type) {
        case TransactionType.EIP712Transaction:
            tx_req.eip712_meta = {
                factory_deps: l2tx.execute.factory_deps,
                custom_signature: l2tx.common_data.signature,
                paymaster_params: null
            };
            break;
        case TransactionType.EIP1559Transaction:
            break;
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

    let signature = signTransactionRequest(signer, tx_req);
    let signed_bytes = rlp_tx(tx_req, signature, chain_id);

    return signed_bytes;
}

function rlp_tx(tx: TransactionRequest, signature: ethers.SignatureLike, chain_id: number) {
    const fields: any[] = [];

    if (tx.type == TransactionType.EIP1559Transaction) {
        fields.push(ethers.toBeArray(chain_id));
    }
    fields.push(ethers.toBeArray(tx.nonce));
    if (tx.to != null) {
        fields.push(ethers.toBeArray(tx.to));
    } 
    fields.push(ethers.toBeArray(tx.input));
    
    // Signature
    fields.push(ethers.toBeArray(signature.v));
    fields.push(ethers.toBeArray(signature.r));
    fields.push(ethers.toBeArray(signature.s));

    // EIP712 || OLA
    if (tx.type == TransactionType.EIP712Transaction || tx.type == TransactionType.OlaRawTransaction) {
        fields.push(ethers.toBeArray(chain_id));
        if (tx.from != null) {
            fields.push(ethers.toBeArray(tx.from));
        }

        if (tx.customData != null) {
            fields.push((tx.customData.factoryDeps ?? []).map((dep) => ethers.hexlify(dep)));
            if (tx.customData.customSignature && ethers.getBytes(tx.customData.customSignature).length == 0) {
                throw new Error("Empty signatures are not supported");
            }
            fields.push(tx.customData.customSignature || "0x");

            if (tx.customData.paymasterParams) {
                fields.push([
                    tx.customData.paymasterParams.paymaster,
                    ethers.hexlify(tx.customData.paymasterParams.paymasterInput),
                ]);
            } else {
                fields.push([]);
            }
        }
    }

    return ethers.concat([new Uint8Array([TransactionType.OlaRawTransaction]), ethers.encodeRlp(fields)]);
}

// TODO: use ECDSA sign the TransactionRequest.
function signTransactionRequest(signer: OlaSigner, transaction_request: TransactionRequest): number[] {
    return [1, 2];
}

export function encodeTransaction(signer: OlaSigner, chain_id: number, from: Address, nonce: number, calldata: string, factory_deps: number[][] | null): L2Tx {
    let req: TransactionRequest = {
        nonce: nonce,
        from: from,
        to: ENTRYPOINT_ADDRESS,
        input: calldata,
        chainId: chain_id
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

export function createCalldata(from: Address, to: Address, abi: string, method: string, params: Record<string, any>[], codes: number[] | null) {
    const abiJson = JSON.parse(abi);
    let biz_calldata = encode_input_from_js(abiJson, method, params);
    let entrypoint_calldata = createEntrypointCalldata(from, to, biz_calldata, codes);
    // entrypoint_calldata is Vec<u64>, should convert to Vec<u8>
    return entrypoint_calldata;
}

function createEntrypointCalldata(from: Address, to: Address, calldata: Array<number>, codes: number[] | null) {
    const entrypointAbiJson = require("./Entrypoint_abi.json");
    const method = "system_entrance";
    const params = [{
            tuple: [
                { Address: from },
                { Address: to },
                { Fields: calldata },
                { Fields: codes }
        ]},
        { Bool: false }
    ];

    let data = encode_input_from_js(entrypointAbiJson, method, params);
    return data;
}

export function parseTx(strTx: string): L2Tx {
    const parsedTx = JSON.parse(strTx) as L2Tx;
    return parsedTx;
}