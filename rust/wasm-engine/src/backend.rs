use crate::{errors::Result, ExportedFunction, Schema, WasmArgs, WasmReturn, WasmValue};

pub trait Backend {
    type Instance<'a>
    where
        Self: 'a;
    type Module<'a>
    where
        Self: 'a;

    fn load_module<'a>(
        &'a self,
        bytes: impl AsRef<[u8]>,
        schema: &Schema,
    ) -> Result<Self::Module<'a>>;

    fn instantiate<'a>(&'a self, module: &Self::Module<'a>) -> Result<Self::Instance<'a>>;

    fn call<'a>(
        &'a self,
        instance: &Self::Instance<'a>,
        name: &str,
        args: WasmArgs,
    ) -> Result<WasmValue>;
}
