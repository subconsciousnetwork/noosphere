use crate::{errors::Result, ExportedFunction, Schema, WasmArgs, WasmReturn, WasmValue};

pub trait Backend {
    type Instance<'a>
    where
        Self: 'a;
    fn instantiate<'a>(
        &'a self,
        bytes: impl AsRef<[u8]>,
        schema: &Schema,
    ) -> Result<Self::Instance<'a>>;

    fn call<'a>(
        &'a self,
        instance: &Self::Instance<'a>,
        name: &str,
        args: WasmArgs,
    ) -> Result<WasmValue>;
}
