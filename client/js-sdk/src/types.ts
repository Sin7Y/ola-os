import { BigNumberish, toBeHex, getUint } from "ethers";

export class U256 {
    private value: BigNumberish;

    constructor(value: BigNumberish) {
        this.value = value;
    }

    toBigNumber(): BigNumberish {
        return this.value;
    }

    toH256(): H256 {
        const hexString = toBeHex(this.value, 32).slice(2).padStart(32, "0");
        const bytes = hexString.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) as number[];
        return new H256(bytes);
    }

    static from(value: BigNumberish): U256 {
        return new U256(value);
    }

    intoU64BigInts(): bigint[] {
        const bigIntValue = this.bigInt();
        // 0xFFFFFFFFFFFFFFFF
        const mask64 = 2n ** 64n - 1n;
        const parts: bigint[] = [];
        for (let i = 0; i < 4; i++) {
            const part = (bigIntValue >> (BigInt(i) * 64n)) & mask64;
            parts.unshift(part);
        }
        return parts;
    }

    bigInt(): bigint {
        return getUint(this.value);
    }
}

export class H256 {
    private bytes: number[];

    constructor(bytes: number[]) {
        this.bytes = bytes;
    }

    toU256(): U256 {
        const hexString = "0x" + this.bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
        const bigNumberValue = BigInt(hexString);
        return new U256(bigNumberValue);
    }

    static from(value: BigNumberish): H256 {
        return U256.from(value).toH256();
    }
}

export type N256 = U256 | H256 | BigNumberish;
