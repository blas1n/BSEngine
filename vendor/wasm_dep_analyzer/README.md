# wasm_dep_analyzer

An extremely lightweight Wasm module parser used in Deno to get the dependencies
of a Wasm module from its bytes for the purpose of ECMAScript module resolution
and TypeScript type checking.

```rs
let deps = WasmDeps::parse(&wasm_module_bytes, ParseOptions::default())?;

eprintln!("{:#?}", deps.imports);
eprintln!("{:#?}", deps.exports);
```
