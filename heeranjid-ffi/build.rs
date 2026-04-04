use std::path::Path;

fn main() {
    // Generate C header bindings
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file("heeranjid.h");

    // Copy SQL files from submodule to output directory when include-sql feature is enabled
    if std::env::var("CARGO_FEATURE_INCLUDE_SQL").is_ok() {
        let sql_src = Path::new(&crate_dir).join("../sql");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let sql_dest = Path::new(&out_dir).join("sql");

        if sql_src.exists() {
            copy_dir_recursive(&sql_src, &sql_dest)
                .expect("Failed to copy SQL files to output directory");
            println!("cargo:warning=SQL files copied to {}", sql_dest.display());
        } else {
            panic!(
                "include-sql feature enabled but sql/ submodule not found at {}",
                sql_src.display()
            );
        }
    }

    // Re-run build script if SQL files change
    println!("cargo:rerun-if-changed=../sql");
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            // Skip .git directories inside the submodule
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}
