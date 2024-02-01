import { toBeArray, toBigInt, toUtf8Bytes, hexlify, getBytes, ethers, SigningKey, sha256, keccak256, toBeHex } from 'ethers';
import { poseidon_u64_bytes_for_bytes_wrapper } from '@sin7y/ola-crypto';
import { encode_input_from_js, decode_output_from_js } from '@sin7y/ola-abi-wasm';
import axios from 'axios';

/**
 * BigNumberish / Uint8Array -> BigUint64Array
 * @param value
 * @returns
 */
function toUint64Array(value) {
    let bytes = value instanceof Uint8Array ? value : toBeArray(value);
    if (bytes.length % 8 !== 0) {
        const remain = 8 - (bytes.length % 8);
        const padding = new Uint8Array(remain).fill(0);
        bytes = new Uint8Array([...padding, ...bytes]);
    }
    const chunkLength = Math.ceil(bytes.length / 8);
    const result = new BigUint64Array(chunkLength);
    for (let i = 0; i < chunkLength; i++) {
        const value = toBigInt(bytes.slice(i * 8, 8 * (i + 1)));
        result[i] = value;
    }
    return result;
}
const maxKeyBoundary = 2n ** 64n - 2n ** 32n + 1n;
function isValidOlaKey(key) {
    return toUint64Array(key).every((item) => item <= maxKeyBoundary);
}
/**
 * convert HexString address -> bigint[]
 * @param address
 * @returns
 */
function toBigintArray(address) {
    return Array.from(toUint64Array(address));
}
/**
 * BigUint64Array -> Uint8Array
 * @param arr
 * @returns
 */
function toUint8Array(value) {
    const length = typeof value === "bigint" ? 1 : value.length;
    const array = typeof value === "bigint" ? [value] : value;
    const buffer = new ArrayBuffer(length * 8); // 8 bytes per Uint64
    const dataView = new DataView(buffer);
    for (let i = 0; i < length; i++) {
        // Set values in big-endian order
        dataView.setBigUint64(i * 8, array[i], false);
    }
    return new Uint8Array(buffer);
}
function poseidonHash(data) {
    return Uint8Array.from(poseidon_u64_bytes_for_bytes_wrapper(data));
}

function getAbiBytes(abi) {
    const abiJson = JSON.stringify(abi);
    return toUtf8Bytes(abiJson);
}
/**
 *
 * @param abi abi array
 * @param method method signature
 * @param params params array
 * @returns BigUint64Array
 */
function encodeAbi(abi, method, params) {
    const result = encode_input_from_js(getAbiBytes(abi), method, params);
    return BigUint64Array.from(result.map((item) => BigInt(item)));
}
function decodeAbi(abi, method, data) {
    const result = decode_output_from_js(getAbiBytes(abi), method, data);
    return result;
}

const ENTRYPOINT_ADDRESS = [0x0n, 0x0n, 0x0n, 0x8001n];
const DEFAULT_CHAIN_ID = 1027;
/**
 * for setPubkey only
 *
 * 0x0000000000000000000000000000000000000000000000000000000000008006
 */
const DEFAULT_ACCOUNT_ADDRESS = [0x0n, 0x0n, 0x0n, 0x8006n];

var TransactionType;
(function (TransactionType) {
    TransactionType[TransactionType["EIP712Transaction"] = 113] = "EIP712Transaction";
    TransactionType[TransactionType["EIP1559Transaction"] = 2] = "EIP1559Transaction";
    TransactionType[TransactionType["OlaRawTransaction"] = 16] = "OlaRawTransaction";
    TransactionType[TransactionType["PriorityOpTransaction"] = 255] = "PriorityOpTransaction";
    TransactionType[TransactionType["ProtocolUpgradeTransaction"] = 254] = "ProtocolUpgradeTransaction";
})(TransactionType || (TransactionType = {}));

const ENTRYPOINT_ABI = [
    {
        name: "system_entrance",
        type: "function",
        inputs: [
            {
                name: "_tx",
                type: "tuple",
                components: [
                    {
                        name: "from",
                        type: "address",
                    },
                    {
                        name: "to",
                        type: "address",
                    },
                    {
                        name: "data",
                        type: "fields",
                    },
                    {
                        name: "codes",
                        type: "fields",
                    },
                ],
            },
            {
                name: "_isETHCall",
                type: "bool",
            },
        ],
        outputs: [],
    },
];

class OlaAddress {
    static toBigintArray(value) {
        const bytes = toUint64Array(value);
        const padCount = 4 - bytes.length;
        const padArray = padCount > 0 ? new BigUint64Array(padCount).fill(0n) : null;
        const result = padArray ? new BigUint64Array([...padArray, ...bytes]) : bytes.slice(0, 4);
        return Array.from(result);
    }
}

function createEntrypointCalldata(from, to, calldata, codes = []) {
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
function txRequestToBytes(tx) {
    if (tx.eip712_meta == null) {
        throw new Error("We can sign transaction only with meta");
    }
    if (tx.eip712_meta.paymaster_params != null &&
        tx.eip712_meta.paymaster_params.paymaster_input.length % 8 != 0) {
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
    let data = [];
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
async function signTransactionRequest(signer, tx) {
    const message = txRequestToBytes(tx);
    const messageHash = poseidonHash(message);
    const signature = signer.signMessage(Uint8Array.from(messageHash));
    const sigBytes = new Uint8Array(65);
    sigBytes.set(toBeArray(signature.r), 0);
    sigBytes.set(toBeArray(signature.s), 32);
    sigBytes[64] = signature.v;
    return sigBytes;
}
async function getL2Tx(signer, chain_id, from, nonce, calldata, factory_deps = null) {
    const fromAddress = Array.from(toUint64Array(from));
    const txRequest = {
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
    const tx = {
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
function l2txToTransactionRequest(l2tx) {
    let chain_id = 1027;
    let tx_type = l2tx.common_data.transaction_type;
    let r = toUint64Array(l2tx.common_data.signature.slice(0, 32));
    let s = toUint64Array(l2tx.common_data.signature.slice(32, 64));
    let v = l2tx.common_data.signature[64];
    let txRequest = {
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
function rlp_tx(tx, signature, chain_id) {
    if (signature.length != 65) {
        throw Error("Signature length must be 65");
    }
    if (tx.type != TransactionType.EIP1559Transaction &&
        tx.type != TransactionType.EIP712Transaction &&
        tx.type != TransactionType.OlaRawTransaction) {
        throw Error("Unknown transaction type");
    }
    const fields = [];
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
    if (tx.type == TransactionType.EIP712Transaction ||
        tx.type == TransactionType.OlaRawTransaction) {
        fields.push(toBeArray(chain_id));
        if (tx.from != null) {
            fields.push(toUint8Array(tx.from));
        }
        if (tx.eip712_meta != null) {
            fields.push((tx.eip712_meta.factory_deps ?? []).map((dep) => hexlify(dep)));
            if (tx.eip712_meta.custom_signature &&
                getBytes(tx.eip712_meta.custom_signature).length == 0) {
                throw new Error("Empty signatures are not supported");
            }
            fields.push(tx.eip712_meta.custom_signature || "0x");
            if (tx.eip712_meta.paymaster_params) {
                fields.push([
                    tx.eip712_meta.paymaster_params.paymaster,
                    hexlify(tx.eip712_meta.paymaster_params.paymaster_input),
                ]);
            }
            else {
                fields.push([]);
            }
        }
    }
    return ethers.concat([
        new Uint8Array([TransactionType.OlaRawTransaction]),
        ethers.encodeRlp(fields),
    ]);
}
async function createSignedTransactionRaw(l2tx, signer, chainId) {
    const txRequest = l2txToTransactionRequest(l2tx);
    const signature = await signTransactionRequest(signer, txRequest);
    const signed_bytes = rlp_tx(txRequest, signature, chainId);
    return signed_bytes;
}
async function createTransaction(signer, chainId, from, nonce, calldata, factory_deps = null) {
    const l2tx = await getL2Tx(signer, chainId, from, nonce, calldata, factory_deps);
    const raw_tx = await createSignedTransactionRaw(l2tx, signer, chainId);
    return raw_tx;
}

/**
 * capitalize the first letter.
 * @param value
 * @returns
 */
function capitalize(value) {
    return value.charAt(0).toUpperCase() + value.slice(1);
}

function computePublicKey(privateKey) {
    return "0x" + SigningKey.computePublicKey(privateKey).slice(4);
}
function privateKeyFromSeed(seed) {
    let privateKey = sha256(seed);
    let count = 0;
    while (count < 10000) {
        let publicKey = computePublicKey(privateKey);
        if (isValidOlaKey(privateKey) && isValidOlaKey(publicKey)) {
            return privateKey;
        }
        else {
            privateKey = keccak256(privateKey);
            count++;
        }
    }
}
class OlaSigner {
    constructor(privateKey) {
        this.privateKey = privateKey;
        this.publicKey = computePublicKey(privateKey);
        const hashBytes = poseidonHash(toBeArray(this.publicKey));
        this.address = hexlify(hashBytes);
    }
    getL2Tx(chain_id, from, nonce, calldata, factory_deps = null) {
        const fromAddress = Array.from(toUint64Array(from));
        const txRequest = {
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
        const tx = {
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
    signMessage(message) {
        if (typeof message === "string" && !message.startsWith("0x")) {
            throw Error("OlaSigner Error: Invalid message. Expected HexString or Uint8Array.");
        }
        const hexMessage = hexlify(message);
        const privKey = new SigningKey(this.privateKey);
        return privKey.sign(hexMessage);
    }
    signTransactionRequest(tx) {
        const message = txRequestToBytes(tx);
        const messageHash = poseidonHash(message);
        const signature = this.signMessage(Uint8Array.from(messageHash));
        const sigBytes = new Uint8Array(65);
        sigBytes.set(toBeArray(signature.r), 0);
        sigBytes.set(toBeArray(signature.s), 32);
        sigBytes[64] = signature.v;
        return sigBytes;
    }
    createSignedTransactionRaw(l2tx, chainId) {
        const txRequest = l2txToTransactionRequest(l2tx);
        const txRequestSig = this.signTransactionRequest(txRequest);
        const rawTx = rlp_tx(txRequest, txRequestSig, chainId);
        return rawTx;
    }
    createTransaction(chainId, nonce, calldata, factory_deps = null) {
        const l2tx = this.getL2Tx(chainId, this.address, nonce, calldata, factory_deps);
        return this.createSignedTransactionRaw(l2tx, chainId);
    }
    static async fromETHSignature(ethSigner) {
        const message = "Access OlaVM.\n" + "\n" + "This account is only for Sepolia testnet.";
        const signature = await ethSigner.signMessage(message);
        const seed = toBeArray(signature);
        const olaPrivateKey = privateKeyFromSeed(seed);
        if (!olaPrivateKey)
            throw new Error("Ola SDK: Private Key generate error.");
        return new OlaSigner(olaPrivateKey);
    }
}

class OlaProvider {
    constructor(url, chainId = DEFAULT_CHAIN_ID) {
        this.chainId = chainId;
        this.baseURL = url.replace(/\/$/, "");
    }
    async health() {
        const res = await axios.get(`${this.baseURL}/health`);
        console.log(res);
    }
    async request(method, params) {
        const requestBody = {
            id: 1,
            jsonrpc: "2.0",
            method,
            params,
        };
        const { data } = await axios.post(this.baseURL, requestBody);
        if (data.error) {
            throw Error(data.error.message);
        }
        return data.result;
    }
    async getNonce(address) {
        return this.request("eth_getTransactionCount", { address });
    }
}

const ACCOUNT_ABI = [
    {
        name: "setPubkey",
        type: "function",
        inputs: [
            {
                name: "_pk",
                type: "fields",
            },
        ],
        outputs: [],
    },
    {
        "name": "getPubkey",
        "type": "function",
        "inputs": [
            {
                "name": "_address",
                "type": "address"
            }
        ],
        "outputs": [
            {
                "name": "",
                "type": "fields"
            }
        ]
    }
];

const DEFAULT_RPC_URL = "/";
class OlaWallet {
    constructor(signer, provider) {
        this.signer = signer;
        this.provider = provider;
        this.chainId = DEFAULT_CHAIN_ID;
    }
    get address() {
        return this.signer.address;
    }
    connect(rpcUrl, chainId) {
        this.chainId = chainId ?? DEFAULT_CHAIN_ID;
        this.provider = new OlaProvider(rpcUrl, chainId);
    }
    async getNonce() {
        return this.provider.getNonce(this.address);
    }
    /**
     *
     * @param abi
     * @param method
     * @param to DataHexString
     * @param params
     * @param options nonce
     * @returns
     */
    async invoke(abi, method, to, params, options) {
        const nonce = options?.nonce ?? (await this.getNonce());
        const bizCalldata = encodeAbi(abi, method, params);
        const entryCalldata = createEntrypointCalldata(this.address, to, bizCalldata);
        const calldata = toUint8Array(entryCalldata);
        const txRaw = this.signer.createTransaction(this.chainId, nonce, calldata, null);
        const txHash = await this.provider.request("ola_sendRawTransaction", {
            tx_bytes: txRaw,
        });
        return txHash;
    }
    async call(abi, method, to, params) {
        const nonce = await this.getNonce();
        const bizCalldata = encodeAbi(abi, method, params);
        // All fields in CallRequest should be hex string.
        const call_request = {
            from: this.address,
            to: to,
            data: hexlify(toUint8Array(bizCalldata)),
            nonce: toBeHex(nonce),
            transaction_type: toBeHex(TransactionType.OlaRawTransaction),
        };
        const tx = await this.provider.request("ola_callTransaction", {
            call_request,
        });
        const decoded = decodeAbi(abi, method, toUint64Array(tx));
        const outputs = decoded[1][0];
        const outputType = outputs.param.type;
        const outputsValue = outputs.value[capitalize(outputType)];
        return outputsValue;
    }
    async setPubKey() {
        return this.invoke(ACCOUNT_ABI, "setPubkey(fields)", hexlify(toUint8Array(DEFAULT_ACCOUNT_ADDRESS)), [{ Fields: toBigintArray(this.signer.publicKey) }], { nonce: 0 });
    }
    static async fromETHSignature(ethSigner, rpcUrl) {
        const signer = await OlaSigner.fromETHSignature(ethSigner);
        const provider = new OlaProvider(rpcUrl ?? DEFAULT_RPC_URL);
        return new OlaWallet(signer, provider);
    }
}

export { DEFAULT_CHAIN_ID, OlaAddress, OlaProvider, OlaSigner, OlaWallet, createEntrypointCalldata, createTransaction, decodeAbi, encodeAbi, poseidonHash, toUint64Array, toUint8Array };
