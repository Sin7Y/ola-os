export enum TransactionType {
  EIP712Transaction = 113,
  EIP1559Transaction = 2,
  OlaRawTransaction = 16,
  PriorityOpTransaction = 255,
  ProtocolUpgradeTransaction = 254,
}

export interface Eip712Meta {
  factory_deps: null | Array<Uint8Array>;
  custom_signature: null | Uint8Array;
  paymaster_params: null | {
    paymaster: bigint[];
    paymaster_input: Uint8Array;
  };
}

export interface TransactionRequest {
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
export interface L2Tx {
  execute: Execute;
  common_data: L2TxCommonData;
  received_timestamp_ms: number;
}

export interface CallResponse {
  jsonrpc: string,
  result: string,
  id:number,
}
