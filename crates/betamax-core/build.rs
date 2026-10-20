use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or("CARGO_MANIFEST_DIR is not set while generating bundled themes")?,
    );
    // Ghostty themes come first so a refreshed Ghostty snapshot wins over a VHS-only import.
    let mut themes: Vec<(String, PathBuf)> = Vec::new();
    for dir in ["resources/ghostty/themes", "resources/vhs/themes"] {
        let themes_dir = manifest_dir.join(dir);
        println!("cargo:rerun-if-changed={}", themes_dir.display());
        for entry in fs::read_dir(&themes_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".DS_Store" || themes.iter().any(|(existing, _)| existing == &name) {
                continue;
            }
            themes.push((name, entry.path()));
        }
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("resources/vhs/aliases.json").display()
    );
    themes.sort_by_key(|(name, _)| name.to_ascii_lowercase());

    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR").ok_or("OUT_DIR is not set while generating bundled themes")?,
    );
    let mut source = String::from("const BUNDLED_THEMES: &[(&str, &str)] = &[\n");
    for (name, path) in themes {
        source.push_str("    (");
        source.push_str(&format!("{name:?}"));
        source.push_str(", include_str!(");
        source.push_str(&format!("{:?}", path.display().to_string()));
        source.push_str(")),\n");
    }
    source.push_str("];\n");

    fs::write(out_dir.join("bundled_themes.rs"), source)?;
    Ok(())
}
