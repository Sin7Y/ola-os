import { encode_input_from_js } from "./crypto/ola_lang_abi"

type H256 = string;

type Address = H256;

const ENTRYPOINT_ADDRESS = "0x8001";

interface Execute {
    contract_address: Address;
    calldata: number[];
    factory_deps?: number[][] | null;
}

interface Nonce {
    nonce:number;
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
    nonce: Nonce;
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

type U256 = [number, number, number, number];

type U64 = [number];

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

// export function createTransactionRaw(l2tx: L2Tx): number[] {

// }

// TODO: use ECDSA sign the TransactionRequest.
function signTransactionRequest(transaction_request: TransactionRequest): number[] {
    return [1, 2];
}

export function encodeTransaction(chain_id: number, from: Address, nonce: Nonce, calldata: number[], factory_deps: number[][] | null): L2Tx {
    let req: TransactionRequest = {
        nonce: [0, 0, 0, nonce.nonce],
        from: from,
        to: ENTRYPOINT_ADDRESS,
        input: calldata,
        transaction_type: TransactionType.OlaRawTransaction,
        eip712_meta: {
            factory_deps: factory_deps
        },
        chain_id: chain_id
    };
    let signature = signTransactionRequest(req);

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