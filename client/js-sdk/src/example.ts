import { Signature } from "ethers";
import { encode_input_from_js } from "./crypto/ola_lang_abi"
import { U256, H256 } from "./types";
import { OlaSigner } from "./signer";
import { RLP } from "@ethereumjs/rlp";

type Address = H256;

const ENTRYPOINT_ADDRESS: H256 = H256.from("0x8001");

interface Execute {
    contract_address: Address;
    calldata: number[];
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
    nonce: U256;
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

type U64 = [number];

type Option<T> = T | null;

type SignatureTuple = [Option<U64>, Option<U256>, Option<U256>];

interface Eip712Meta {
    factory_deps?: number[][] | null;
    custom_signature?: number[] | null;
    paymaster_params?: PaymasterParams | null;
}

interface PaymasterParams {
    paymaster: Address;
    paymaster_input: number[];
}

interface TransactionRequest {
    nonce: U256;
    from?: Address | null;
    to?: Address | null;
    input: number[];
    v?: U64 | null;
    r?: U256 | null;
    s?: U256 | null;
    raw?: number[] | null;
    transaction_type?: TransactionType | null;
    eip712_meta?: Eip712Meta | null;
    chain_id?: number | null;
}

export function createTransactionRaw(l2tx: L2Tx, signer: OlaSigner): number[] {
    // TODO: chainid should be extract from common_data
    let chain_id = 1027;
    let tx_type = l2tx.common_data.transaction_type;
    let r = l2tx.common_data.signature.slice(0,32).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    let s = l2tx.common_data.signature.slice(32,64).map((byte) => byte.toString(16).padStart(2, "0")).join("");
    let tx_req: TransactionRequest = {
        nonce: l2tx.common_data.nonce,
        from: l2tx.common_data.initiator_address,
        input: l2tx.execute.calldata,
        v: [0],
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

    let tobeEncode: Buffer[] = [];
    switch (tx_type) {
        case TransactionType.EIP1559Transaction:
            // tobeEncode.push(Buffer.from(chain_id));
            break;
    }

    return [1]
}

// TODO: use ECDSA sign the TransactionRequest.
function signTransactionRequest(signer: OlaSigner, transaction_request: TransactionRequest): number[] {
    return [1, 2];
}

export function encodeTransaction(signer: OlaSigner, chain_id: number, from: Address, nonce: U256, calldata: number[], factory_deps: number[][] | null): L2Tx {
    let req: TransactionRequest = {
        nonce: nonce,
        from: from,
        to: ENTRYPOINT_ADDRESS,
        input: calldata,
        transaction_type: TransactionType.OlaRawTransaction,
        eip712_meta: {
            factory_deps: factory_deps
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

export function createCalldata(from: Uint8Array, to: Uint8Array, abi: string, method: string, params: Record<string, any>[], codes: number[] | null) {
    const abiJson = JSON.parse(abi);
    let biz_calldata = encode_input_from_js(abiJson, method, params);
    let entrypoint_calldata = createEntrypointCalldata(from, to, biz_calldata, codes);
    return entrypoint_calldata;
}

function createEntrypointCalldata(from: Uint8Array, to: Uint8Array, calldata: Array<number>, codes: number[] | null) {
    const entrypointAbiStr = "xxxx";
    const entrypointAbiJson = JSON.parse(entrypointAbiStr);
    const method = "system_entrance";
    const params = [
        { Address: from },
        { Address: to },
        { Fields: calldata },
        { Fields: codes },
    ];

    let data = encode_input_from_js(entrypointAbiJson, method, params);
    return data;
}

export function parseTx(strTx: string): L2Tx {
    const parsedTx = JSON.parse(strTx) as L2Tx;
    return parsedTx;
}