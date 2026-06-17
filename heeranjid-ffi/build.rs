use std::path::{Path, PathBuf};

fn main() {
    // Generate C header bindings
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("Unable to generate C bindings")
        .write_to_file(format!("{out_dir}/heeranjid.h"));

    // Copy SQL files from submodule to output directory when include-sql feature is enabled
    if std::env::var("CARGO_FEATURE_INCLUDE_SQL").is_ok() {
        let sql_src = Path::new(&crate_dir).join("../sql");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let sql_dest = Path::new(&out_dir).join("sql");

        if sql_src.exists() {
            let sql_src_canonical = sql_src
                .canonicalize()
                .expect("Failed to canonicalize SQL source directory");
            copy_dir_recursive(&sql_src_canonical, &sql_src_canonical, &sql_dest)
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

fn copy_dir_recursive(src_root: &Path, src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;

        // Do not follow symlinks to avoid escaping the trusted source tree.
        if metadata.file_type().is_symlink() {
            continue;
        }

        let canonical_path: PathBuf = path.canonicalize()?;
        if !canonical_path.starts_with(src_root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Path escapes SQL source directory: {}", canonical_path.display()),
            ));
        }

        let dest_path = dst.join(entry.file_name());
        if canonical_path.is_dir() {
            // Skip .git directories inside the submodule
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive(src_root, &canonical_path, &dest_path)?;
        } else {
            std::fs::copy(&canonical_path, &dest_path)?;
        }
    }
    Ok(())
}
