(module
  (import "" "i_add" (func $i_add (param i32) (param i32) (result i32)))
  (func (export "e_add") (param $1 i32) (param $2 i32) (result i32)
    local.get $1
    local.get $2
    (call $i_add))
)
