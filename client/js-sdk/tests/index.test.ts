import { OlaSigner, encodeAbi, decodeAbi, OlaWallet, createEntrypointCalldata, createTransaction, toUint8Array, toUint64Array } from "../src";
import { ethers, hexlify, toBeArray, toUtf8Bytes } from "ethers";
import { expect } from "chai";

// describe("L2TX parse Test", () => {
//   it("L2TX parse", async () => {
//     const tx = '{"execute":{"contractAddress":"0x0101010101010101010101010101010101010101010101010101010101010101","calldata":[1,2,3],"factoryDeps":[[6,6,6],[6,6,6]]},"common_data":{"nonce":4,"initiator_address":"0x0202020202020202020202020202020202020202020202020202020202020202","signature":[7,8,9],"transaction_type":"OlaRawTransaction","input":{"hash":"0x0505050505050505050505050505050505050505050505050505050505050505","data":[4,4,4]}},"received_timestamp_ms":1706278213739}';

//     const par = parseTx(tx);
//     console.log(par);

//     const l2txInstance: L2Tx = {
//       execute: {
//           contract_address: "0x1234567890123456789012345678901234567890",
//           calldata: [1, 2, 3, 4],
//       },
//       common_data: {
//           nonce: { nonce: 1 },
//           initiator_address: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef",
//           signature: [5, 6, 7, 8],
//           transaction_type: TransactionType.OlaRawTransaction,
//       },
//       received_timestamp_ms: Date.now(),
//   };

//   console.log(l2txInstance);
//   });
// });

// describe("H256/U256 encode Test", () => {
//   it("H256/U256 encode", async () => {
//     const from = H256.from(BigInt(908173248920127022929968509872062022378588115024631874819275168689514742274n));
//     console.log("from: ", from);

//     const x = BigInt(908173248920127022929968509872062022378588115024631874819275168689514742274n);
//     console.log(x);
//     console.log(x.toString(16));
//     const newArray = Array.from({ length: 32 }, () => 2);
//     console.log(newArray);
//     const bigIntValue = newArray.reduce((acc, byte, index) => acc | (BigInt(byte) << BigInt((31 - index) * 8)), BigInt(0));
//     console.log(bigIntValue.toString(16));
//   });
// });

// describe("L2TX encode Test", () => {
//   it("L2TX encode", async () => {
//     const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
//     const ethWallet = new ethers.Wallet(pk);
//     // console.log("random wallet", ethWallet.address);
//     const olaSigner = await OlaSigner.fromETHSignature(ethWallet);

//     const tx = encodeTransaction(olaSigner, 100, H256.from("0x202020202020202020202020202020202020202020202020202020202020202"), U256.from(1), [2, 3, 4], null);
//     console.log(tx);
//     console.log("from: ", tx.common_data.initiator_address);

//   });
// });

// describe("Transaction Encode Test", () => {
//   it("encode", async () => {
//     // await init();
//     const from_raw = [123, 123, 123, 123];
//     const to_raw = [456n, 456n, 456n, 456n];
//     const from: OlaAddress = new BigUint64Array([123n, 123n, 123n, 123n]);
//     const to: OlaAddress = new BigUint64Array(to_raw);
//     const abi = `[
//       {
//         "name": "setVote",
//         "type": "function",
//         "inputs": [
//           {
//             "name": "_address",
//             "type": "address"
//           },
//           {
//             "name": "_vote",
//             "type": "u32"
//           }
//         ],
//         "outputs": []
//       },
//       {
//         "name": "vote_for",
//         "type": "function",
//         "inputs": [],
//         "outputs": []
//       }
//     ]`;
//     const params = [{ Address: [10, 10, 10, 10] }, { U32: 123 }];
//     const method = "setVote(address,u32)";
//     const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
//     const ethWallet = new ethers.Wallet(pk);
//     const olaSigner = await OlaSigner.fromETHSignature(ethWallet);

//     const encoder = new TextEncoder();
//     const uint8Array = encoder.encode(abi);
//     const jsonString = JSON.stringify([...uint8Array]);
//     const calldata = await createCalldata(from, to, jsonString, method, params, null);
//     const raw = createTransaction(olaSigner, 1027, from, 1, calldata, null);
//     console.log("raw tx: ", raw);
//     // Now we can use provider to send the raw transaction
//   });
// });

// describe("ABI Test", async () => {
//   it("encode", async () => {
//     const abi = [
//       {
//         name: "createBook",
//         type: "function",
//         inputs: [
//           {
//             name: "id",
//             type: "u32",
//             internalType: "u32",
//           },
//           {
//             name: "name",
//             type: "string",
//             internalType: "string",
//           },
//         ],
//         outputs: [
//           {
//             name: "",
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
//       },
//     ];
//     const method = "createBook(u32,string)";
//     const params = [{ U32: 60 }, { String: "olavm" }];
//     const result = await encodeAbi(abi, method, params);
//     console.log(result);
//     // const url = new URL("index.test.ts", import.meta.url);
//     // console.log(url);
//     // const res = await fetch(url.href);
//     // console.log(res);
//   });
// });

// describe("Example Test", () => {
//   it("should pass", () => {
//     expect(true).to.equal(true);
//   });
// });

describe("Wallet Test", () => {
  it("Create Account", async () => {
    // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
    const ethPrivateKey = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
    const ethWallet = new ethers.Wallet(ethPrivateKey);
    const olaWallet = await OlaWallet.fromETHSignature(ethWallet);

    expect(olaWallet.signer.publicKey).to.eq("0x4dfe4a76a9260db664a4b7c8a3b5293364507c3857e9457ac84f9ca36a9c9c7c4243c6405ca2c8a5b1e62766dc77f2f90ff54e70bb49995d28fb8f98782e005c");
    expect(olaWallet.address).to.eq("0xc32eff4be49142ea8ec271e65126a2cc4f227ebed16b62a7388222bd5afb3e0f");
  });
});

describe("ABI Encoder Test", () => {
  it("Encode ABI", async () => {
    const abi = [
      {
        name: "createBook",
        type: "function",
        inputs: [
          { name: "id", type: "u32", internalType: "u32" },
          { name: "name", type: "string", internalType: "string" },
        ],
        outputs: [
          {
            name: "",
            type: "tuple",
            internalType: "struct BookExample.Book",
            components: [
              { name: "book_id", type: "u32", internalType: "u32" },
              { name: "book_name", type: "string", internalType: "string" },
            ],
          },
        ],
      },
    ];
    const method = "createBook(u32,string)";
    const params = [{ U32: 60 }, { String: "olavm" }];
    const result = await encodeAbi(abi, method, params);
    expect(result).to.deep.eq(new BigUint64Array([60n, 5n, 111n, 108n, 97n, 118n, 109n, 7n, 120553111n]));
  });

  it("Decode ABI", async () => {
    const abi = [
      {
        name: "getBookName",
        type: "function",
        inputs: [
          {
            name: "_book",
            type: "tuple",
            internalType: "struct BookExample.Book",
            components: [
              {
                name: "book_id",
                type: "u32",
                internalType: "u32",
              },
              {
                name: "book_name",
                type: "string",
                internalType: "string",
              },
            ],
          },
        ],
        outputs: [
          {
            name: "",
            type: "string",
            internalType: "string",
          },
        ],
      },
    ];
    const data = new BigUint64Array([5n, 104n, 101n, 108n, 108n, 111n, 6n]);
    const method = "getBookName((u32,string))";
    const result = await decodeAbi(abi, method, data);
    expect(result).to.deep.eq([
      {
        name: "getBookName",
        inputs: [
          {
            name: "_book",
            type: "tuple",
            components: [
              { name: "book_id", type: "u32" },
              { name: "book_name", type: "string" },
            ],
          },
        ],
        outputs: [{ name: "", type: "string" }],
      },
      [
        {
          param: { name: "", type: "string" },
          value: { String: "hello" },
        },
      ],
    ]);
  });
});

describe("Transaction Encode Test", () => {
  it("encode", async () => {
    const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
    const ethWallet = new ethers.Wallet(pk);
    const olaWallet = await OlaWallet.fromETHSignature(ethWallet);

    const abi = [
      {
        name: "setVote",
        type: "function",
        inputs: [
          { name: "_address", type: "address" },
          { name: "_vote", type: "u32" },
        ],
        outputs: [],
      },
    ];
    const method = "setVote(address,u32)";
    const params = [{ Address: [10n, 10n, 10n, 10n] }, { U32: 123 }];
    const bizCalldata = await encodeAbi(abi, method, params);

    // [123n, 123n, 123n, 123n]
    // const from = "0x000000000000007b000000000000007b000000000000007b000000000000007b";
    const from = olaWallet.address;
    // [456n, 456n, 456n, 456n]
    const to = "0x00000000000001c800000000000001c800000000000001c800000000000001c8";
    const nonce = 1;
    const chainId = 1027;
    const entryCalldata = await createEntrypointCalldata(from, to, bizCalldata);
    const calldata = toUint8Array(entryCalldata);
    const raw = await createTransaction(olaWallet.signer, chainId, from, nonce, calldata);
    console.log("raw", raw);

    // Now we can use provider to send the raw transaction
  });
});
