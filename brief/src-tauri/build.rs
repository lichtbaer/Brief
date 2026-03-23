fn main() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let models_dir = std::path::Path::new(&manifest_dir).join("../whisperx_runner/models");

    if !models_dir.exists() {
        let _ = std::fs::create_dir_all(&models_dir);
        println!(
            "cargo:warning=whisperx_runner/models/ was missing — created it. \
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

    // Override the relative resource glob with an absolute path so the build
    // succeeds regardless of CWD. TAURI_CONFIG is deep-merged into tauri.conf.json
    // by tauri_build::build().
    if std::env::var("TAURI_CONFIG").is_err() {
        if let Ok(abs_models) = models_dir.canonicalize() {
            let glob = format!("{}/**/*", abs_models.display());
            let override_cfg = format!(
                r#"{{"bundle":{{"resources":["{}"]}}}}"#,
                glob.replace('\\', "/")
            );
            unsafe { std::env::set_var("TAURI_CONFIG", &override_cfg) };
        }
    }

    tauri_build::build()
}
