fn main() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let models_dir = std::path::Path::new(&manifest_dir).join("../whisperx_runner/models");

    if !models_dir.exists() {
        let _ = std::fs::create_dir_all(&models_dir);
        println!(
            "cargo:warning=whisperx_runner/models/ was missing, created it. \
             Run `cd whisperx_runner && bash setup.sh` to download models."
        );
    }

    // Glob ** doesn't match hidden files — use a non-hidden placeholder
    let placeholder = models_dir.join("README");
    if !placeholder.exists() {
        let _ = std::fs::write(
            &placeholder,
            "Models are downloaded during setup.\nRun: cd whisperx_runner && bash setup.sh\n",
        );
    }

    tauri_build::build()
}
