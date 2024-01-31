import axios from "axios";
import { DEFAULT_CHAIN_ID } from "./constants";

export class OlaProvider {
  public baseURL: string;

  constructor(url: string, public chainId: number = DEFAULT_CHAIN_ID) {
    this.baseURL = url.replace(/\/$/, "");
  }

  async health() {
    const res = await axios.get(`${this.baseURL}/health`);
    console.log(res);
  }

  async request<T>(method: string, params: Record<string, any> | null) {
    const requestBody = {
      id: 1,
      jsonrpc: "2.0",
      method,
      params,
    };

    const { data } = await axios.post(this.baseURL, requestBody);
    console.log("response data", data);
    return data as T;
  }

  async getNonce(address: string) {
    return this.request<number>("eth_getTransactionCount", { address });
  }
}
