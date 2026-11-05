use std::{collections::HashSet, env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides the crate directory"),
    );
    let feature_dir = manifest_dir.join("src/features");
    println!("cargo:rerun-if-changed={}", feature_dir.display());

    let mut feature_files = fs::read_dir(&feature_dir)
        .expect("read built-in feature modules")
        .map(|entry| entry.expect("read feature directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    feature_files.sort();
    assert!(
        !feature_files.is_empty(),
        "at least one built-in feature is required"
    );

    let mut generated = String::from("// Generated from src/features by build.rs.\n");
    for path in &feature_files {
        println!("cargo:rerun-if-changed={}", path.display());
        let module = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("feature filenames are valid UTF-8");
        assert!(
            module
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
            "feature module names must use snake_case: {module}"
        );
        generated.push_str(&format!(
            "#[path = {:?}]\nmod {module};\n",
            path.display().to_string()
        ));
    }

    generated.push_str("\nimpl Feature {\n");
    let mut feature_names = HashSet::new();
    for path in &feature_files {
        let module = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("feature filenames are valid UTF-8");
        let feature_name = module
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<String>();
        assert!(
            feature_names.insert(feature_name.clone()),
            "feature module names must generate unique Feature constants: {feature_name}"
        );
        let key = module.replace('_', "-");
        generated.push_str(&format!(
            "    #[allow(non_upper_case_globals)]\n    pub const {feature_name}: Self = Self({key:?});\n"
        ));
    }
    generated.push_str("}\n\nconst FEATURES: &[FeatureDefinition] = &[\n");
    for path in &feature_files {
        let module = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("feature filenames are valid UTF-8");
        generated.push_str(&format!("    {module}::DEFINITION,\n"));
    }
    generated.push_str("];\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides an output path"))
        .join("feature_registry.rs");
    fs::write(output, generated).expect("write generated built-in feature registry");
}
