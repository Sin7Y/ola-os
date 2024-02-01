import typescript from "@rollup/plugin-typescript";
import { dts } from "rollup-plugin-dts";

export default [
  {
    input: "src/index.ts",
    output: [
      { file: "dist/index.es.js", format: "es" },
      { file: "dist/index.cjs.js", format: "cjs" },
    ],
    external: ["ethers", "axios", "@sin7y/ola-abi-wasm", "@sin7y/ola-crypto"],
    plugins: [typescript()],
  },
  {
    input: "src/index.ts",
    output: [{ file: "dist/index.d.ts", format: "es" }],
    plugins: [dts()],
  },
];
