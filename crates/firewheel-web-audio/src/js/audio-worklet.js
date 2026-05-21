registerProcessor(
  "WasmProcessor",
  class WasmProcessor extends AudioWorkletProcessor {
    constructor(options) {
      super();
      let [module, memory, handle] = options.processorOptions;
      bindgen.initSync(module, memory);
      this.processor = bindgen.ProcessorHost.unpack(handle);
    }

    process(inputs, outputs) {
      return this.processor.process(inputs, outputs, currentTime);
    }
  },
);
