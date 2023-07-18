use std::marker::PhantomData;

use crate::Error;
use anyhow::Result;

pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(u32),
    F64(u64),
    V128(u128),
    FuncRef,
    ExternRef,
}

pub enum WasmType {
    I32,
    I64,
}

impl From<i32> for WasmValue {
    fn from(value: i32) -> Self {
        WasmValue::I32(value)
    }
}

impl TryFrom<WasmValue> for i32 {
    type Error = Error;
    fn try_from(value: WasmValue) -> Result<Self, Self::Error> {
        match value {
            WasmValue::I32(x) => Ok(x),
            _ => Err(Error::Other("Cannot cast into i32".into())),
        }
    }
}
impl From<i64> for WasmValue {
    fn from(value: i64) -> Self {
        WasmValue::I64(value)
    }
}

pub struct WasmArgs {
    values: Vec<WasmValue>,
}

impl WasmArgs {
    pub fn new(values: Vec<WasmValue>) -> Self {
        WasmArgs { values }
    }

    pub fn into_inner(self) -> Vec<WasmValue> {
        self.values
    }
}

impl<T: Into<WasmValue>> From<T> for WasmArgs {
    fn from(value: T) -> Self {
        WasmArgs::new(vec![value.into()])
    }
}

impl<T: Into<WasmValue>, const N: usize> From<[T; N]> for WasmArgs {
    fn from(array: [T; N]) -> Self {
        WasmArgs::new(array.into_iter().map(|v| v.into()).collect())
    }
}

pub trait WasmParams: Send {}
impl<T> WasmParams for T where T: Into<WasmValue> + Send {}
impl<T> WasmParams for (T, T) where T: Into<WasmValue> + Send {}

pub trait WasmReturn {}
impl<T> WasmReturn for T where T: Into<WasmValue> {}

pub trait Signature {}

struct ImportedFunction<A, R> {
    name: String,
    args: A,
    ret: R,
}

pub struct Schema {
    pub(crate) imports: (),
    pub(crate) exports: Vec<Box<ExportedFunction<dyn WasmParams, dyn WasmReturn>>>,
}

impl Schema {
    pub fn new(imports: (), exports: Vec<Box<ExportedFunction<_, _>>>) -> Self {
        Schema { imports, exports }
    }
}

pub struct ExportedFunction<Params: WasmParams, Return: WasmReturn> {
    name: String,
    _marker: std::marker::PhantomData<Params>,
    _marker2: std::marker::PhantomData<Return>,
}

impl<Params: WasmParams, Return: WasmReturn> ExportedFunction<Params, Return> {
    pub fn new(name: &str) -> Self {
        ExportedFunction {
            name: name.to_owned(),
            _marker: PhantomData,
            _marker2: PhantomData,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
/* pub struct ExportedFunction<Params: WasmArgs + ?Sized, Return: WasmReturn + ?Sized> {
    name: String,
    args: P,
    _marker: std::marker::PhantomData<*const (Params, Return)>,
} */

/*
macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!(0);
        $mac!(1 A1);
        $mac!(2 A1 A2);
        $mac!(3 A1 A2 A3);
        $mac!(4 A1 A2 A3 A4);
        $mac!(5 A1 A2 A3 A4 A5);
        $mac!(6 A1 A2 A3 A4 A5 A6);
        $mac!(7 A1 A2 A3 A4 A5 A6 A7);
        $mac!(8 A1 A2 A3 A4 A5 A6 A7 A8);
        $mac!(9 A1 A2 A3 A4 A5 A6 A7 A8 A9);
        $mac!(10 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10);
        $mac!(11 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11);
        $mac!(12 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12);
        $mac!(13 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13);
        $mac!(14 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14);
        $mac!(15 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15);
        $mac!(16 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15 A16);
    };
}

macro_rules! impl_into_func {
    ($num:tt $($args:ident)*) => {
        impl<T, F, $($args,)* R> IntoFunc<T, ($($args,)*), R> for F
        where
            F: Fn($($args),*) -> R + Send + Sync + 'static,
            $($args: WasmTy,)*
            R: WasmRet,
        {
            // ...
        }
    }
}

for_each_function_signature!(impl_into_func);

impl WasmArgs for (WasmType) {}
impl WasmArgs for (WasmType, WasmType) {}
impl WasmArgs for (WasmType, WasmType, WasmType) {}

pub struct Schema {}

trait FooScheme {
    fn hello() {}
}

struct ImportedFunction<A, R> {
    name: String,
    args: A,
    ret: R,
}

impl ImportedFunction<A, R> {
    pub fn new(name: &str) -> Self {
        ImportedFunction {
            name: name.to_owned(),
        }
    }
}

//ImportedFunction<(i32, i32), i32>::new("add");

#[macro_export]
macro_rules! wasm_imports {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}
*/
