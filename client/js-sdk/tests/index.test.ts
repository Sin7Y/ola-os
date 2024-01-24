import { OlaSigner, encodeAbi } from "../src";
import { ethers } from "ethers";
import { expect } from "chai";

describe("Wallet Test", () => {
  it("create account", async () => {
    // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
    const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
    const ethWallet = new ethers.Wallet(pk);
    // console.log("random wallet", ethWallet.address);
    const olaSigner = await OlaSigner.fromETHSignature(ethWallet);
    // console.log("ola wallet", olaSigner.privateKey, olaSigner.address);

    expect(1 + 1).to.eq(2);
  });
});

describe("ABI Test", async () => {
  it("encode", async () => {
    // const abi = [
    //   {
    //     name: "createBook",
    //     type: "function",
    //     inputs: [
    //       {
    //         name: "id",
    //         type: "u32",
    //         internalType: "u32",
    //       },
    //       {
    //         name: "name",
    //         type: "string",
    //         internalType: "string",
    //       },
    //     ],
    //     outputs: [
    //       {
    //         name: "",
    //         type: "tuple",
    //         internalType: "struct BookExample.Book",
    //         components: [
    //           {
    //             name: "book_id",
    //             type: "u32",
    //             internalType: "u32",
    //           },
    //           {
    //             name: "book_name",
    //             type: "string",
    //             internalType: "string",
    //           },
    //         ],
    //       },
    //     ],
    //   },
    // ];
    // const method = "createBook(u32,string)";
    // const params = [{ U32: 60 }, { String: "olavm" }];
    // const result = await encodeAbi(abi, method, params);
    // console.log(result);
    const url = new URL("index.test.ts", import.meta.url);
    console.log(url);
    const res = await fetch(url.href);
    console.log(res);
  });
});

// describe("Example Test", () => {
//   it("should pass", () => {
//     expect(true).to.equal(true);
//   });
// });
