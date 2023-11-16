use vergen::EmitBuilder;

fn main() {
    EmitBuilder::builder()
        .all_git()
        .all_build()
        .all_cargo()
        .emit()
        .unwrap();
}
