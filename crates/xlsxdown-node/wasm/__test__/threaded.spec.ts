import * as binding from "@d0paminedriven/xlsxdown-wasm";

import { wasmSuite } from "@/wasm/__test__/suite.ts";

// A caller who names no build gets the one with threads, and the type says
// so: it is that build's name, and `native` is not among what it can be.
wasmSuite(binding, binding.__napiBindingTarget satisfies "wasm32-wasi");
