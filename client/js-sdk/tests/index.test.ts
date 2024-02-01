import { encodeAbi, decodeAbi, OlaWallet } from "../src";
import { ethers } from "ethers";
import { expect } from "chai";

async function generateAccount() {
  // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
  const ethPrivateKey = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";

  /**
   * Browser:
   * ethWallet = new ethers.BrowserProvider(window.ethereum)
   */
  const ethWallet = new ethers.Wallet(ethPrivateKey);
  const olaWallet = await OlaWallet.fromETHSignature(ethWallet);
  // @note: connect to provider
  olaWallet.connect("https://pre-alpha-api.olavm.com:443", 1027);
  return olaWallet;
}

function sleep(time: number) {
  it(`sleep ${time}ms`, function (done) {
    setTimeout(() => done(), time);
  });
}

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
    expect(result).to.deep.eq(
      new BigUint64Array([60n, 5n, 111n, 108n, 97n, 118n, 109n, 7n, 120553111n])
    );
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

describe("Wallet Test", () => {
  it("Create Account", async () => {
    const olaWallet = await generateAccount();
    expect(olaWallet.signer.publicKey).to.eq(
      "0x4dfe4a76a9260db664a4b7c8a3b5293364507c3857e9457ac84f9ca36a9c9c7c4243c6405ca2c8a5b1e62766dc77f2f90ff54e70bb49995d28fb8f98782e005c"
    );
    expect(olaWallet.address).to.eq(
      "0xc32eff4be49142ea8ec271e65126a2cc4f227ebed16b62a7388222bd5afb3e0f"
    );
  });

  it("setPubKey()", async () => {
    try {
      const olaWallet = await generateAccount();
      const txHash = await olaWallet.setPubKey();
      console.log(txHash);
    } catch (error: any) {
      console.log(error.message);
    }
  });

  const contracAddress = "0x26d5e4afcc2c1dcec2385e164e40d2bcb14384e9e74f46d4b9d626654d13bcf9";
  it("invoke()", async () => {
    const olaWallet = await generateAccount();
    const abi = [
      { name: "set", type: "function", inputs: [{ name: "d", type: "u32" }], outputs: [] },
    ];
    const params = [{ U32: 2000 }];
    const txHash = await olaWallet.invoke(abi, "set(u32)", contracAddress, params);
    console.log("invoke txHash", txHash);
  });

  sleep(1000);

  it("call()", async () => {
    const olaWallet = await generateAccount();
    const abi = [
      { name: "get", type: "function", inputs: [], outputs: [{ name: "", type: "u32" }] },
    ];
    let result = await olaWallet.call<number>(abi, "get()", contracAddress, []);
    expect(result).eq(2000);
  });
});
