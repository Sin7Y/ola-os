export const ENTRYPOINT_ABI = [
  {
    name: "system_entrance",
    type: "function",
    inputs: [
      {
        name: "_tx",
        type: "tuple",
        components: [
          {
            name: "from",
            type: "address",
          },
          {
            name: "to",
            type: "address",
          },
          {
            name: "data",
            type: "fields",
          },
          {
            name: "codes",
            type: "fields",
          },
        ],
      },
      {
        name: "_isETHCall",
        type: "bool",
      },
    ],
    outputs: [],
  },
];
