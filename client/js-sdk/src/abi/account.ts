export const ACCOUNT_ABI = [
  {
    name: "setPubkey",
    type: "function",
    inputs: [
      {
        name: "_pk",
        type: "fields",
      },
    ],
    outputs: [],
  },
  {
    "name": "getPubkey",
    "type": "function",
    "inputs": [
      {
        "name": "_address",
        "type": "address"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "fields"
      }
    ]
  }
];
