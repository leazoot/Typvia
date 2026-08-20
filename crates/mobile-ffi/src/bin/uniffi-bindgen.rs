// Library-mode bindgen entry point: bindings are generated from
// compiled library metadata, so no UDL file exists to drift out of sync.
fn main() {
    uniffi::uniffi_bindgen_main()
}
