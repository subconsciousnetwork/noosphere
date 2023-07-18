use std::any::Any;

use crate::{Backend, ExportedFunction, Result, Schema, WasmArgs, WasmReturn, WasmValue};

pub struct Module<'a, B: Backend> {
    backend: &'a B,
    internals: B::Module<'a>,
}

impl<'a, B: Backend> Module<'a, B> {
    pub fn new(engine: &'a Engine<B>, bytes: impl AsRef<[u8]>, schema: &Schema) -> Result<Self> {
        let backend = &engine.backend;
        let internals = backend.load_module(bytes, schema)?;
        Ok(Module { backend, internals })
    }

    pub fn instantiate(&self) -> Result<Instance<B>> {
        Instance::new(self)
    }
}

pub struct Instance<'a, B: Backend> {
    backend: &'a B,
    internals: B::Instance<'a>,
}

impl<'a, B: Backend> Instance<'a, B> {
    pub fn new(module: &'a Module<B>) -> Result<Self> {
        let backend = &module.backend;
        let internals = backend.instantiate(module)?;
        Ok(Instance { backend, internals })
    }

    pub fn call(&self, name: &str, args: impl Into<WasmArgs>) -> Result<WasmValue> {
        self.backend.call(&self.internals, name, args.into())
    }
}

pub struct Engine<B: Backend> {
    backend: B,
}

impl<B: Backend> Engine<B> {
    pub fn new(backend: B) -> Self {
        Engine { backend }
    }

    pub fn load_module(&self, bytes: &[u8], schema: &Schema) -> Result<Module<B>> {
        Module::new(self, bytes, schema)
    }
}
