use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let sdkconfig_defaults = PathBuf::from(&manifest_dir).join("sdkconfig.defaults");

    // Tell cargo to rerun if sdkconfig.defaults changes
    println!("cargo:rerun-if-changed=sdkconfig.defaults");

    // Check if the environment variable is set (it should be by justfile)
    let env_set = env::var("ESP_IDF_SDKCONFIG_DEFAULTS").is_ok();
    if !env_set {
        eprintln!("WARNING: ESP_IDF_SDKCONFIG_DEFAULTS not set! Stack size may be wrong.");
        eprintln!("Make sure to build with: export ESP_IDF_SDKCONFIG_DEFAULTS=crates/xteink-firmware/sdkconfig.defaults");
    }

    // CRITICAL: Find and check the cached sdkconfig in target directory
    // If sdkconfig.defaults is newer, we must force regeneration
    let target_dir = PathBuf::from(&manifest_dir).join("target");
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("build").exists() {
                // Look for esp-idf-sys build directories
                if let Ok(build_entries) = fs::read_dir(path.join("build")) {
                    for build_entry in build_entries.flatten() {
                        let build_path = build_entry.path();
                        let build_path_str = build_path.to_string_lossy();
                        if build_path_str.contains("esp-idf-sys") {
                            let sdkconfig = build_path.join("out/esp-idf/sdkconfig");
                            if sdkconfig.exists() && sdkconfig_defaults.exists() {
                                let sdkconfig_modified =
                                    fs::metadata(&sdkconfig).and_then(|m| m.modified()).ok();
                                let defaults_modified = fs::metadata(&sdkconfig_defaults)
                                    .and_then(|m| m.modified())
                                    .ok();

                                if let (Some(sdk_time), Some(defaults_time)) =
                                    (sdkconfig_modified, defaults_modified)
                                {
                                    if defaults_time > sdk_time {
                                        eprintln!(
                                            "sdkconfig.defaults changed! Forcing regeneration..."
                                        );
                                        // Delete sdkconfig to force regeneration
                                        let _ = fs::remove_file(&sdkconfig);
                                        // Also delete the sdkconfig.d directory
                                        let _ = fs::remove_dir_all(
                                            build_path.join("out/esp-idf/sdkconfig.d"),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    embuild::espidf::sysenv::output();
}
