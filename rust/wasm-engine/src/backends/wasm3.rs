#![cfg(feature = "wasm3")]
use crate::{Backend, Error, Result};
use wasm3;

pub struct ModuleWrapper {
    internals: wasm3::ParsedModule,
}

pub struct Wasm3Backend {
    env: wasm3::Environment,
    runtime: wasm3::Runtime,
}

impl Wasm3Backend {
    pub fn new() -> Result<Self> {
        let env = wasm3::Environment::new()?;
        let runtime = env.create_runtime(1024)?;

        Ok(Wasm3Backend { env, runtime })
    }
}

impl Backend for Wasm3Backend {
    type Instance<'a> = wasm3::Module<'a>;

    fn instantiate<'a>(
        &'a self,
        bytes: impl AsRef<[u8]>,
        schema: &Schema,
    ) -> Result<Self::Instance<'a>> {
        let instance = self.runtime.parse_and_load_module(bytes.as_ref())?;
        Ok(instance)
    }

    fn call<'a>(
        &'a self,
        instance: &Self::Instance<'a>,
        name: &str,
        args: WasmArgs,
    ) -> Result<WasmValue> {
    }
}
/*
let hello_func =
    wasmtime::Func::wrap(&mut store, |mut caller: wasmtime::Caller<'_, MyState>| {
        println!("Calling back...");
        println!("> {}", caller.data().name);
        caller.data_mut().count += 1;
    });

let imports = [hello_func.into()];
let instance = wasmtime::Instance::new(&mut store, &module, &imports)?;

println!("Extracting export...");
let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

println!("Calling export...");
run.call(&mut store, ())?;

println!("Done.");
Ok(())
*/

impl From<wasm3::error::Error> for Error {
    fn from(value: wasm3::error::Error) -> Self {
        value.to_string().into()
    }
}
