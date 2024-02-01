import {
  OlaSigner,
  encodeAbi,
  decodeAbi,
  OlaWallet,
  createEntrypointCalldata,
  createTransaction,
  toUint8Array,
  toUint64Array,
  DEFAULT_ACCOUNT_ADDRESS,
  OlaAddress,
} from "../src";
import { ethers, hexlify, toBeArray, toUtf8Bytes } from "ethers";
import { expect } from "chai";

async function generateAccount() {
  // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
  const ethPrivateKey = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
  const ethWallet = new ethers.Wallet(ethPrivateKey);
  const olaWallet = await OlaWallet.fromETHSignature(ethWallet);
  // @note: connect to provider
  olaWallet.connect("https://pre-alpha-api.olavm.com:443", 1027);
  return olaWallet;
}

// describe("Wallet & Setpubkey Test", () => {
//   it("Create Account", async () => {
//     const olaWallet = await generateAccount();
//     expect(olaWallet.signer.publicKey).to.eq(
//       "0x4dfe4a76a9260db664a4b7c8a3b5293364507c3857e9457ac84f9ca36a9c9c7c4243c6405ca2c8a5b1e62766dc77f2f90ff54e70bb49995d28fb8f98782e005c"
//     );
//     expect(olaWallet.address).to.eq(
//       "0xc32eff4be49142ea8ec271e65126a2cc4f227ebed16b62a7388222bd5afb3e0f"
//     );

//     let tx = await olaWallet.setPubKey();
//     console.log(tx);
//   });
// });

describe("Wallet & Invoke Test", () => {
  it("Invoke transaction", async () => {
    const olaWallet = await generateAccount();
    expect(olaWallet.signer.publicKey).to.eq(
      "0x4dfe4a76a9260db664a4b7c8a3b5293364507c3857e9457ac84f9ca36a9c9c7c4243c6405ca2c8a5b1e62766dc77f2f90ff54e70bb49995d28fb8f98782e005c"
    );
    expect(olaWallet.address).to.eq(
      "0xc32eff4be49142ea8ec271e65126a2cc4f227ebed16b62a7388222bd5afb3e0f"
    );

    const foo_abi =[
      {
        "name": "set",
        "type": "function",
        "inputs": [
          {
            "name": "d",
            "type": "u32"
          }
        ],
        "outputs": []
      },
      {
        "name": "get",
        "type": "function",
        "inputs": [],
        "outputs": [
          {
            "name": "",
            "type": "u32"
          }
        ]
      }
    ];
    const contrac_address = "0x26d5e4afcc2c1dcec2385e164e40d2bcb14384e9e74f46d4b9d626654d13bcf9";
    const params = [
      {U32: 2000}
    ];

    let tx = await olaWallet.invoke(foo_abi, "set(u32)", contrac_address, params);
    console.log(tx);
  });
});


// describe("Call Test", () => {
//   it("Call data", async () => {
//     const olaWallet = await generateAccount();
//     expect(olaWallet.signer.publicKey).to.eq(
//       "0x4dfe4a76a9260db664a4b7c8a3b5293364507c3857e9457ac84f9ca36a9c9c7c4243c6405ca2c8a5b1e62766dc77f2f90ff54e70bb49995d28fb8f98782e005c"
//     );
//     expect(olaWallet.address).to.eq(
//       "0xc32eff4be49142ea8ec271e65126a2cc4f227ebed16b62a7388222bd5afb3e0f"
//     );

//     const foo_abi =[
//       {
//         "name": "set",
//         "type": "function",
//         "inputs": [
//           {
//             "name": "d",
//             "type": "u32"
//           }
//         ],
//         "outputs": []
//       },
//       {
//         "name": "get",
//         "type": "function",
//         "inputs": [],
//         "outputs": [
//           {
//             "name": "",
//             "type": "u32"
//           }
//         ]
//       }
//     ];
//     const contrac_address = "0x26d5e4afcc2c1dcec2385e164e40d2bcb14384e9e74f46d4b9d626654d13bcf9";

//     let tx = await olaWallet.call(foo_abi, "get()", contrac_address, []);
//     console.log(tx);
//   });
// });

// describe("ABI Encoder Test", () => {
//   it("Encode ABI", async () => {
//     const abi = [
//       {
//         name: "createBook",
//         type: "function",
//         inputs: [
//           { name: "id", type: "u32", internalType: "u32" },
//           { name: "name", type: "string", internalType: "string" },
//         ],
//         outputs: [
//           {
//             name: "",
//             type: "tuple",
//             internalType: "struct BookExample.Book",
//             components: [
//               { name: "book_id", type: "u32", internalType: "u32" },
//               { name: "book_name", type: "string", internalType: "string" },
//             ],
//           },
//         ],
//       },
//     ];
//     const method = "createBook(u32,string)";
//     const params = [{ U32: 60 }, { String: "olavm" }];
//     const result = await encodeAbi(abi, method, params);
//     expect(result).to.deep.eq(
//       new BigUint64Array([60n, 5n, 111n, 108n, 97n, 118n, 109n, 7n, 120553111n])
//     );
//   });

//   it("Decode ABI", async () => {
//     const abi = [
//       {
//         name: "getBookName",
//         type: "function",
//         inputs: [
//           {
//             name: "_book",
//             type: "tuple",
//             internalType: "struct BookExample.Book",
//             components: [
//               {
//                 name: "book_id",
//                 type: "u32",
//                 internalType: "u32",
//               },
//               {
//                 name: "book_name",
//                 type: "string",
//                 internalType: "string",
//               },
//             ],
//           },
//         ],
//         outputs: [
//           {
//             name: "",
//             type: "string",
//             internalType: "string",
//           },
//         ],
//       },
//     ];
//     const data = new BigUint64Array([5n, 104n, 101n, 108n, 108n, 111n, 6n]);
//     const method = "getBookName((u32,string))";
//     const result = await decodeAbi(abi, method, data);
//     expect(result).to.deep.eq([
//       {
//         name: "getBookName",
//         inputs: [
//           {
//             name: "_book",
//             type: "tuple",
//             components: [
//               { name: "book_id", type: "u32" },
//               { name: "book_name", type: "string" },
//             ],
//           },
//         ],
//         outputs: [{ name: "", type: "string" }],
//       },
//       [
//         {
//           param: { name: "", type: "string" },
//           value: { String: "hello" },
//         },
//       ],
//     ]);
//   });
// });

// describe("Transaction Encode Test", () => {
//   it("encode", async () => {
//     const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
//     const ethWallet = new ethers.Wallet(pk);
//     const olaWallet = await OlaWallet.fromETHSignature(ethWallet);

//     const abi = [
//       {
//         name: "setVote",
//         type: "function",
//         inputs: [
//           { name: "_address", type: "address" },
//           { name: "_vote", type: "u32" },
//         ],
//         outputs: [],
//       },
//     ];
//     const method = "setVote(address,u32)";
//     const params = [{ Address: [10n, 10n, 10n, 10n] }, { U32: 123 }];
//     const bizCalldata = await encodeAbi(abi, method, params);

//     const from = olaWallet.address;
//     // [456n, 456n, 456n, 456n]
//     const to = "0x00000000000001c800000000000001c800000000000001c800000000000001c8";
//     const nonce = 1;
//     const chainId = 1027;
//     const entryCalldata = await createEntrypointCalldata(from, to, bizCalldata);
//     const calldata = toUint8Array(entryCalldata);
//     const raw = await createTransaction(olaWallet.signer, chainId, from, nonce, calldata);
//     // console.log("raw", raw);

//     // olaWallet.invoke({
//     //   abi,
//     //   method,
//     //   params,
//     //   to
//     // })

//     // Now we can use provider to send the raw transaction
//   });
// });

// describe("Provider Test", async () => {
//   const olaWallet = await generateAccount();

//   it("getNonce()", async () => {
//     const nonce = await olaWallet.getNonce();
//     console.log("nonce", nonce);
//   });
// });
