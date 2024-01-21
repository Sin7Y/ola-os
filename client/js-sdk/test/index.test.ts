import { OlaSigner } from "../src";
import { ethers } from "ethers";

test("Init", async () => {
  // @note: address - '0x54253578fFc18424a174DC81Ab98c43b654752F6'
  const pk = "0xead3c88c32e5938420ae67d7e180005512aee9eb7ab4ebedff58f95f4ef06504";
  const ethWallet = new ethers.Wallet(pk);
  console.log("random wallet", ethWallet.address);
  const olaSigner = await OlaSigner.fromETHSignature(ethWallet);
  console.log("ola wallet", olaSigner.privateKey, olaSigner.address);

  expect(1 + 1).toBe(2);
});
