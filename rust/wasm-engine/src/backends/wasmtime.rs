//#![cfg(feature = "wasmtime")]
use crate::{
    Backend, Error, ExportedFunction, Result, Schema, WasmArgs, WasmParams, WasmReturn, WasmValue,
};
use std::any::Any;
use std::ops::DerefMut;
use std::sync::Mutex;
use wasmtime;

struct State {}

pub struct ModuleWrapper {
    internals: wasmtime::Module,
}

pub struct InstanceWrapper {
    internals: wasmtime::Instance,
    store: Mutex<wasmtime::Store<State>>,
}

pub struct WasmtimeBackend {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<State>,
}

impl WasmtimeBackend {
    pub fn new() -> Result<Self> {
        let engine = wasmtime::Engine::default();
        let linker = wasmtime::Linker::new(&engine);

        Ok(WasmtimeBackend { engine, linker })
    }
    /*
       fn store_function<Params: WasmParams + ?Sized, Return: WasmReturn + ?Sized>(
           &self,
           instance: &InstanceWrapper,
           func: &ExportedFunction<Params, Return>,
       ) {
           instance
               .internals
               .get_typed_func::<Params, Return>(&mut instance.store, func.name());
       }
    */
}

impl Backend for WasmtimeBackend {
    type Instance<'a> = InstanceWrapper;
    type Module<'a> = wasmtime::Module;

    fn load_module<'a>(
        &'a self,
        bytes: impl AsRef<[u8]>,
        schema: &Schema,
    ) -> Result<Self::Module<'a>> {
        let internals = wasmtime::Module::from_binary(&self.engine, bytes.as_ref())
            .map_err(|e| <wasmtime::Error as Into<Error>>::into(e))?;

        for fn_spec in schema.exports {
            //self.store_function(&instance, &fn_spec)
        }

        Ok(ModuleWrapper { internals })
    }

    fn instantiate<'a>(&'a self, module: &Self::Module<'a>) -> Result<Self::Instance<'a>> {
        let mut store = wasmtime::Store::new(&self.engine, State {});

        let internals = self
            .linker
            .instantiate(&mut store, &module.internals)
            .map_err(|e| <wasmtime::Error as Into<Error>>::into(e))?;

        let instance = InstanceWrapper {
            store: Mutex::new(store),
            internals,
        };

        Ok(instance)
    }

    fn call<'a>(
        &'a self,
        instance: &Self::Instance<'a>,
        name: &str,
        args: WasmArgs,
    ) -> Result<WasmValue> {
        let mut store = instance
            .store
            .lock()
            .map_err(|_| Error::from("could not acquire mutex"))?;
        let func = instance
            .internals
            .get_func(store.deref_mut(), name)
            .ok_or_else(|| Error::from("No function with that name."))?;
        let args: Vec<wasmtime::Val> = args.into();
        let mut results: Vec<wasmtime::Val> = vec![wasmtime::Val::I32(0)];
        println!("ARGS! {:#?}", args);
        func.call(store.deref_mut(), &args, &mut results)
            .map_err(|e| {
                println!("ERROR {:#?}", e);
                <String as Into<Error>>::into(e.to_string())
            })?;
        println!("CALL! {:#?}", results);
        results.reverse();
        Ok(results.pop().unwrap().into())
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
impl From<wasmtime::Error> for Error {
    fn from(value: wasmtime::Error) -> Self {
        value.to_string().into()
    }
}

impl From<wasmtime::Val> for WasmValue {
    fn from(value: wasmtime::Val) -> Self {
        match value {
            wasmtime::Val::I32(x) => WasmValue::I32(x),
            wasmtime::Val::I64(x) => WasmValue::I64(x),
            wasmtime::Val::F32(x) => WasmValue::F32(x),
            wasmtime::Val::F64(x) => WasmValue::F64(x),
            wasmtime::Val::V128(x) => WasmValue::V128(x),
            wasmtime::Val::FuncRef(..) => WasmValue::FuncRef,
            wasmtime::Val::ExternRef(..) => WasmValue::ExternRef,
        }
    }
}

impl From<WasmValue> for wasmtime::Val {
    fn from(value: WasmValue) -> Self {
        match value {
            WasmValue::I32(x) => wasmtime::Val::I32(x),
            WasmValue::I64(x) => wasmtime::Val::I64(x),
            WasmValue::F32(x) => wasmtime::Val::F32(x),
            WasmValue::F64(x) => wasmtime::Val::F64(x),
            WasmValue::V128(x) => wasmtime::Val::V128(x),
            WasmValue::FuncRef => wasmtime::Val::FuncRef(None),
            WasmValue::ExternRef => wasmtime::Val::ExternRef(None),
        }
    }
}

impl From<WasmArgs> for Vec<wasmtime::Val> {
    fn from(value: WasmArgs) -> Self {
        value
            .into_inner()
            .into_iter()
            .map(Into::<wasmtime::Val>::into)
            .collect()
    }
}
