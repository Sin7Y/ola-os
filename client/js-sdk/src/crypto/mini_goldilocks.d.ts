/* tslint:disable */
/* eslint-disable */
/**
* @param {BigUint64Array} inputs
* @returns {any[]}
*/
export function poseidon_u64_wrapper(inputs: BigUint64Array): any[];
/**
* @param {Uint8Array} inputs
* @returns {any[]}
*/
export function poseidon_u64_bytes_wrapper(inputs: Uint8Array): any[];
/**
* @param {BigUint64Array} inputs
* @returns {any}
*/
export function poseidon_u64_for_bytes_wrapper(inputs: BigUint64Array): any;
/**
* @param {Uint8Array} inputs
* @returns {any}
*/
export function poseidon_u64_bytes_for_bytes_wrapper(inputs: Uint8Array): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly poseidon_u64_wrapper: (a: number, b: number, c: number) => void;
  readonly poseidon_u64_bytes_wrapper: (a: number, b: number, c: number) => void;
  readonly poseidon_u64_for_bytes_wrapper: (a: number, b: number) => number;
  readonly poseidon_u64_bytes_for_bytes_wrapper: (a: number, b: number) => number;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {SyncInitInput} module
*
* @returns {InitOutput}
*/
export function initSync(module: SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {InitInput | Promise<InitInput>} module_or_path
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: InitInput | Promise<InitInput>): Promise<InitOutput>;
