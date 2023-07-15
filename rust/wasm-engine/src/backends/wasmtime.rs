use anyhow::Result;
use wasmtime;

struct WasmEngine {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker,
    store: wasmtime::Store,
    modules: HashMap<String, wasmtime::Module>,
}

impl WasmEngine {
    pub fn new() -> Self {
      let engine = wasmtime::Engine::default();
      let linker = wasmtime::Linker::new(&engine);
      let mut store = wasmtime::Store::new(
        &engine,
        MyState {
            name: "hello, world!".to_string(),
            count: 0,
        },
    );

        WasmEngine {
            engine,
            linker,
            store,
            modules: Default::default(),
        }
    }

    pub fn load_from_source(&self, name: &str, source: &str) -> Result<()> {
        unimplemented!();
    }

    pub fn load_from_file(&self, name: &str, file: &str) -> Result<()> {
        let module = wasmtime::Module::from_file(&self.engine, file)?;
        self.modules.set(name, module);
        Ok(())
    }

    pub fn instantiate(&self, name: &str) -> Result<()> {
        let module = self.modules.get(name)
        self.linker.instantiate(self.store, )
        Ok(())
    }
}
    
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