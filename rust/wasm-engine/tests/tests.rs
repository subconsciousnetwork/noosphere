use anyhow::Result;
use wasm_engine::{Backend, Engine, ExportedFunction, Instance, Schema};

#[cfg(feature = "wasmtime")]
use wasm_engine::backends::WasmtimeBackend;
#[cfg(feature = "wasmtime")]
type SelectedBackend = WasmtimeBackend;

#[cfg(feature = "wasm3")]
use wasm_engine::backends::Wasm3Backend;
#[cfg(feature = "wasm3")]
type SelectedBackend = Wasm3Backend;

#[test]
fn basic_test() -> Result<()> {
    pub trait GcdModule {
        fn gcd(&self, a: i32, b: i32) -> i32;
    }

    impl<'a, B: Backend> GcdModule for Instance<'a, B> {
        fn gcd(&self, a: i32, b: i32) -> i32 {
            32 //self.call("gcd", (i32))
        }
    }

    let bytes = wat::parse_bytes(include_bytes!("gcd.wat"))?;
    //let schema = Schema::new((), vec![ExportedFunction::<(i32, i32), i32>::new("gcd")]);
    let schema = Schema::new((), vec![]);
    let engine = Engine::new(SelectedBackend::new()?);
    let module = Module::new(&engine, bytes, &schema)?;
    let instance = Instance::new(&engine)?;

    let result = instance.call("gcd", [16, 24])?;
    assert_eq!(TryInto::<i32>::try_into(result)?, 8i32);
    //let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;
    //run.call(&mut store, ())?;

    Ok(())
}
