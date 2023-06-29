fn main() {
    #[cfg(not(feature = "uniffi-bindgen"))]
    panic!("Enable feature 'uniffi-bindgen'.");
    #[cfg(feature = "uniffi-bindgen")]
    uniffi::uniffi_bindgen_main()
}
