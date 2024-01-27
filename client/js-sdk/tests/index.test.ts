import { OlaSigner, encodeAbi, TransactionType, L2Tx, parseTx, encodeTransaction, H256, U256 } from "../src";
import { ethers } from "ethers";
import { expect } from "chai";

// describe("Wallet Test", () => {
//   it("create account", async () => {
//     // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
//     const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
//     const ethWallet = new ethers.Wallet(pk);
//     // console.log("random wallet", ethWallet.address);
//     const olaSigner = await OlaSigner.fromETHSignature(ethWallet);
//     // console.log("ola wallet", olaSigner.privateKey, olaSigner.address);

//     expect(1 + 1).to.eq(2);
//   });
// });

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

describe("H256/U256 encode Test", () => {
  it("H256/U256 encode", async () => {
    const from = H256.from(BigInt(908173248920127022929968509872062022378588115024631874819275168689514742274n));
    console.log("from: ", from);

    const x = BigInt(908173248920127022929968509872062022378588115024631874819275168689514742274n);
    console.log(x);
    console.log(x.toString(16));
    const newArray = Array.from({ length: 32 }, () => 2);
    console.log(newArray);
    const bigIntValue = newArray.reduce((acc, byte, index) => acc | (BigInt(byte) << BigInt((31 - index) * 8)), BigInt(0));
    console.log(bigIntValue.toString(16));
  });
});

describe("L2TX encode Test", () => {
  it("L2TX encode", async () => {
    const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
    const ethWallet = new ethers.Wallet(pk);
    // console.log("random wallet", ethWallet.address);
    const olaSigner = await OlaSigner.fromETHSignature(ethWallet);

    const tx = encodeTransaction(olaSigner, 100, H256.from("0x202020202020202020202020202020202020202020202020202020202020202"), U256.from(1), [2, 3, 4], null);
    console.log(tx);
    console.log("from: ", tx.common_data.initiator_address);
  });
});

// describe("ABI Test", async () => {
//   it("encode", async () => {
//     // const abi = [
//     //   {
//     //     name: "createBook",
//     //     type: "function",
//     //     inputs: [
//     //       {
//     //         name: "id",
//     //         type: "u32",
//     //         internalType: "u32",
//     //       },
//     //       {
//     //         name: "name",
//     //         type: "string",
//     //         internalType: "string",
//     //       },
//     //     ],
//     //     outputs: [
//     //       {
//     //         name: "",
//     //         type: "tuple",
//     //         internalType: "struct BookExample.Book",
//     //         components: [
//     //           {
//     //             name: "book_id",
//     //             type: "u32",
//     //             internalType: "u32",
//     //           },
//     //           {
//     //             name: "book_name",
//     //             type: "string",
//     //             internalType: "string",
//     //           },
//     //         ],
//     //       },
//     //     ],
//     //   },
//     // ];
//     // const method = "createBook(u32,string)";
//     // const params = [{ U32: 60 }, { String: "olavm" }];
//     // const result = await encodeAbi(abi, method, params);
//     // console.log(result);
//     const url = new URL("index.test.ts", import.meta.url);
//     console.log(url);
//     const res = await fetch(url.href);
//     console.log(res);
//   });
// });

// describe("Example Test", () => {
//   it("should pass", () => {
//     expect(true).to.equal(true);
//   });
// });
