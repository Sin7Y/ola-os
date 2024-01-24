import axios from "axios";

export class OlaProvider {
  public baseURL: string;
  constructor(url: string) {
    this.baseURL = url.replace(/\/$/, "");
  }

  async health() {
    const res = await axios.get(`${this.baseURL}/health`);
    console.log(res);
  }

  async request(method: string, params: Record<string, any> | null) {
    const requestBody = {
      id: 1,
      jsonrpc: "2.0",
      method,
      params,
    };

    const response = await axios.post(this.baseURL, requestBody).then((res) => {
      return res.data;
    });

    console.log("response", response);
  }

  async getNonce(address: string) {
    return this.request("eth_getTransactionCount", { address });
  }
}
