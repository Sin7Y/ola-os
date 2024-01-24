/* tslint:disable */
/* eslint-disable */
/**
* @param {Uint8Array} file_content
* @param {BigUint64Array} data
* @returns {any}
*/
export function decode_abi_wrapper(file_content: Uint8Array, data: BigUint64Array): any;
/**
* @param {Uint8Array} file_content
* @param {BigUint64Array} data
* @returns {any}
*/
export function decode_input_from_js(file_content: Uint8Array, data: BigUint64Array): any;
/**
* @param {Uint8Array} file_content
* @param {string} signature
* @param {any} params
* @returns {any}
*/
export function encode_input_from_js(file_content: Uint8Array, signature: string, params: any): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly decode_abi_wrapper: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly decode_input_from_js: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly encode_input_from_js: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
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
