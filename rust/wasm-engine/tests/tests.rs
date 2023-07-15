use anyhow::Result;
use wasm_engine::WasmEngine;

#[test]
fn basic_test() -> Result<()> {
    let engine = WasmEngine::new();
    // instantiated modules and other items like host functions. A Store
    // contains an arbitrary piece of host information, and we use `MyState`
    // here.
    println!("Initializing...");
    let mut store = Store::new(
        &engine,
        MyState {
            name: "hello, world!".to_string(),
            count: 0,
        },
    );

    // Our wasm module we'll be instantiating requires one imported function.
    // the function takes no parameters and returns no results. We create a host
    // implementation of that function here, and the `caller` parameter here is
    // used to get access to our original `MyState` value.
    println!("Creating callback...");
    let hello_func =
        wasmtime::Func::wrap(&mut store, |mut caller: wasmtime::Caller<'_, MyState>| {
            println!("Calling back...");
            println!("> {}", caller.data().name);
            caller.data_mut().count += 1;
        });

    // Once we've got that all set up we can then move to the instantiation
    // phase, pairing together a compiled module as well as a set of imports.
    // Note that this is where the wasm `start` function, if any, would run.
    println!("Instantiating module...");
    let imports = [hello_func.into()];
    let instance = wasmtime::Instance::new(&mut store, &module, &imports)?;

    // Next we poke around a bit to extract the `run` function from the module.
    println!("Extracting export...");
    let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

    // And last but not least we can call it!
    println!("Calling export...");
    run.call(&mut store, ())?;

    println!("Done.");
    Ok(())
}
