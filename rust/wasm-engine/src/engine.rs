use std::any::Any;

use crate::{Backend, ExportedFunction, Result, Schema, WasmArgs, WasmReturn, WasmValue};

pub struct Instance<'a, B: Backend> {
    backend: &'a B,
    internals: B::Instance<'a>,
}

impl<'a, B: Backend> Instance<'a, B> {
    pub fn new(engine: &'a Engine<B>, bytes: impl AsRef<[u8]>, schema: &Schema) -> Result<Self> {
        let backend = &engine.backend;
        let internals = backend.instantiate(bytes, schema)?;
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

    pub fn instantiate(&self, bytes: &[u8], schema: &Schema) -> Result<Instance<B>> {
        Instance::new(self, bytes, schema)
    }
}
