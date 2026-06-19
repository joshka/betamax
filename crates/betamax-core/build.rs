use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or("CARGO_MANIFEST_DIR is not set while generating bundled themes")?,
    );
    let themes_dir = manifest_dir.join("resources/ghostty/themes");
    println!("cargo:rerun-if-changed={}", themes_dir.display());

    let mut themes = Vec::new();
    for entry in fs::read_dir(&themes_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        themes.push((name, entry.path()));
    }
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
