import { BytesLike, ethers, BigNumberish } from 'ethers';

declare enum TransactionType {
    EIP712Transaction = 113,
    EIP1559Transaction = 2,
    OlaRawTransaction = 16,
    PriorityOpTransaction = 255,
    ProtocolUpgradeTransaction = 254
}
interface Eip712Meta {
    factory_deps: null | Array<Uint8Array>;
    custom_signature: null | Uint8Array;
    paymaster_params: null | {
        paymaster: bigint[];
        paymaster_input: Uint8Array;
    };
}
interface TransactionRequest {
    nonce: number;
    from?: bigint[];
    to?: bigint[];
    input: Uint8Array;
    v?: number;
    r?: BigUint64Array;
    s?: BigUint64Array;
    raw?: Uint8Array;
    type?: number;
    eip712_meta?: Eip712Meta;
    chain_id?: number;
}
interface Execute {
    contract_address: bigint[];
    calldata: Uint8Array;
    factory_deps: null | Array<Uint8Array>;
}
interface L2TxCommonData {
    nonce: number;
    initiator_address: bigint[];
    signature: Uint8Array;
    transaction_type: TransactionType;
    input?: {
        hash: Uint8Array;
        data: Uint8Array;
    };
}
interface L2Tx {
    execute: Execute;
    common_data: L2TxCommonData;
    received_timestamp_ms: number;
}

declare class OlaSigner {
    readonly privateKey: BytesLike;
    readonly publicKey: BytesLike;
    readonly address: string;
    private constructor();
    getL2Tx(chain_id: number, from: string, nonce: number, calldata: Uint8Array, factory_deps?: Array<Uint8Array> | null): L2Tx;
    signMessage(message: string | Uint8Array): ethers.Signature;
    signTransactionRequest(tx: TransactionRequest): Uint8Array;
    createSignedTransactionRaw(l2tx: L2Tx, chainId: number): string;
    createTransaction(chainId: number, nonce: number, calldata: Uint8Array, factory_deps?: Array<Uint8Array> | null): string;
    static fromETHSignature(ethSigner: ethers.Signer): Promise<OlaSigner>;
}

declare class OlaProvider {
    chainId: number;
    baseURL: string;
    constructor(url: string, chainId?: number);
    health(): Promise<void>;
    request<T>(method: string, params: Record<string, any> | null): Promise<T>;
    getNonce(address: string): Promise<number>;
}

declare class OlaWallet {
    signer: OlaSigner;
    provider: OlaProvider;
    chainId: number;
    private constructor();
    get address(): string;
    connect(rpcUrl: string, chainId?: number): void;
    getNonce(): Promise<number>;
    /**
     *
     * @param abi
     * @param method
     * @param to DataHexString
     * @param params
     * @param options nonce
     * @returns
     */
    invoke(abi: Array<any>, method: string, to: string, params: Array<any>, options?: {
        nonce: number;
    }): Promise<string>;
    call<T>(abi: Array<any>, method: string, to: string, params: Array<any>): Promise<T>;
    setPubKey(): Promise<string>;
    static fromETHSignature(ethSigner: ethers.Signer, rpcUrl?: string): Promise<OlaWallet>;
}

/**
 * BigNumberish / Uint8Array -> BigUint64Array
 * @param value
 * @returns
 */
declare function toUint64Array(value: Uint8Array | BigNumberish): BigUint64Array;
/**
 * BigUint64Array -> Uint8Array
 * @param arr
 * @returns
 */
declare function toUint8Array(value: BigUint64Array | bigint[] | bigint): Uint8Array;
declare function poseidonHash(data: Uint8Array): Uint8Array;

/**
 *
 * @param abi abi array
 * @param method method signature
 * @param params params array
 * @returns BigUint64Array
 */
declare function encodeAbi(abi: any[], method: string, params: Record<string, any>[]): BigUint64Array;
declare function decodeAbi(abi: any[], method: string, data: BigUint64Array): any;

declare function createEntrypointCalldata(from: string, to: string, calldata: BigUint64Array, codes?: number[]): BigUint64Array;
declare function createTransaction(signer: OlaSigner, chainId: number, from: string, nonce: number, calldata: Uint8Array, factory_deps?: Array<Uint8Array> | null): Promise<string>;

declare class OlaAddress {
    static toBigintArray(value: Uint8Array | BigNumberish): bigint[];
}

declare const DEFAULT_CHAIN_ID = 1027;

export { DEFAULT_CHAIN_ID, OlaAddress, OlaProvider, OlaSigner, OlaWallet, createEntrypointCalldata, createTransaction, decodeAbi, encodeAbi, poseidonHash, toUint64Array, toUint8Array };
