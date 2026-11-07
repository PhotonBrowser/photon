use std::path::{Path, PathBuf};

use photon_brand::{ENGINE_NAME, NAME};

use super::{build, linux_desktop};
use crate::support::{invoke, invoke_with_environment, output, path, stage, success};

struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn package(
    root: &Path,
    release: bool,
    output_directory: &Path,
    verbose: bool,
) -> Result<PathBuf, String> {
    let profile = if release { "release" } else { "debug" };
    let build_command = if release {
        "photon build"
    } else {
        "photon build --debug"
    };
    let app = build::photon_executable(root, release);
    let engine = root.join("build").join(format!("engine-{profile}"));
    if !app.is_file() {
        return Err(format!(
            "{} is missing; run `{build_command}` first",
            app.display()
        ));
    }
    let expected = build::engine_fingerprint(root, release)?;
    let fingerprint = std::fs::read_to_string(engine.join(".photon-build-fingerprint"));
    if fingerprint.ok() != Some(expected) {
        return Err(format!(
            "the {ENGINE_NAME} {profile} build in {} is incomplete or older than the Engine checkout; run `{build_command}` first",
            engine.display()
        ));
    }

    let application_stamp = build::app_build_stamp(root, release)?;
    if std::fs::read_to_string(app.with_extension("photon-build-stamp")).ok()
        != Some(application_stamp.clone())
    {
        return Err(format!(
            "the application build provenance is missing or stale; run `{build_command}` first"
        ));
    }
    if output_directory == root
        || output_directory.starts_with(&engine)
        || output_directory.starts_with(root.join("target"))
    {
        return Err(
            "the output folder must be separate from the checkout root and build inputs".into(),
        );
    }

    let product = NAME.to_lowercase();
    let version = env!("CARGO_PKG_VERSION");
    let (commit, dirty) = revision(root, Some(output_directory))?;
    let debug = if release { "" } else { "-debug" };
    let name = format!(
        "{product}-{version}-{}{dirty}-linux-x86_64{debug}",
        &commit[..12]
    );
    let mut provenance = format!(
        "version={version}\nprofile={profile}\ntarget=x86_64-linux-gnu\n{product}={commit}{dirty}\n"
    );
    for (label, repository) in [("Engine", "Engine"), ("gpui-ce", "vendor/gpui-ce")] {
        let (commit, dirty) = revision(&root.join(repository), None)?;
        provenance.push_str(&format!("{label}={commit}{dirty}\n"));
    }
    let archive_name = format!("{name}.tar.xz");
    let checksum_name = format!("{archive_name}.sha256");

    std::fs::create_dir_all(output_directory)
        .map_err(|error| format!("cannot create {}: {error}", output_directory.display()))?;
    for file in [&archive_name, &checksum_name] {
        let destination = output_directory.join(file);
        if destination.exists() {
            return Err(format!(
                "{} already exists; move it or choose another --output",
                destination.display()
            ));
        }
    }
    let staging_path = output_directory.join(format!(".{name}.{}", std::process::id()));
    std::fs::create_dir(&staging_path)
        .map_err(|error| format!("cannot create {}: {error}", staging_path.display()))?;
    let staging = Staging(staging_path);
    let folder = staging.0.join(&name);

    stage(&format!("Stage the {NAME} {profile} build"));
    stage_build(root, &engine, release, &app, &folder, verbose)?;
    let executable = Path::new("bin").join(app.file_name().unwrap_or_default());
    linux_desktop::write_package_files(&folder, &executable)?;
    std::fs::write(folder.join("VERSION"), provenance)
        .map_err(|error| format!("cannot write the package VERSION file: {error}"))?;
    let readme = include_str!("../../../../packaging/linux/README.in")
        .replace("@NAME@", NAME)
        .replace("@PRODUCT@", &product)
        .replace("@EXECUTABLE@", &executable.to_string_lossy())
        .replace("@DATA_FOLDER@", photon_brand::DATA_FOLDER);
    std::fs::write(folder.join("README"), readme)
        .map_err(|error| format!("cannot write the package README: {error}"))?;
    for (source, destination) in [
        (root.join("LICENSE"), folder.join("LICENSE")),
        (
            root.join("vendor/gpui-ce/LICENSE.md"),
            folder.join("share/licenses/gpui-ce/LICENSE.md"),
        ),
    ] {
        if let Some(directory) = destination.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
        }
        std::fs::copy(&source, &destination)
            .map_err(|error| format!("cannot copy {}: {error}", source.display()))?;
    }

    if build::app_build_stamp(root, release)? != application_stamp {
        return Err(
            "application inputs changed during staging; finish the build and package again".into(),
        );
    }
    stage("Archive the package");
    invoke_with_environment(
        "tar",
        &[
            "--sort=name",
            "--owner=0",
            "--group=0",
            "--numeric-owner",
            "-cJf",
            &archive_name,
            &name,
        ],
        &staging.0,
        verbose,
        &[("XZ_OPT", "-T0".to_owned())],
    )?;
    let checksum = output("sha256sum", &[&archive_name], &staging.0)?;
    std::fs::write(staging.0.join(&checksum_name), checksum)
        .map_err(|error| format!("cannot write the archive checksum: {error}"))?;
    let archive_destination = output_directory.join(&archive_name);
    std::fs::hard_link(staging.0.join(&archive_name), &archive_destination).map_err(|error| {
        format!("cannot publish {archive_name} without replacing an existing file: {error}")
    })?;
    if let Err(error) = std::fs::hard_link(
        staging.0.join(&checksum_name),
        output_directory.join(&checksum_name),
    ) {
        let _ = std::fs::remove_file(&archive_destination);
        return Err(format!("cannot publish {checksum_name}: {error}"));
    }

    let archive = output_directory.join(archive_name);
    success(&format!("Packaged {}", archive.display()));
    println!(
        "This package is experimental and its runtime is not verified. Rust crate and font notices are not collected yet; resolve them before publishing it."
    );
    Ok(archive)
}

fn revision(repository: &Path, excluded: Option<&Path>) -> Result<(String, &'static str), String> {
    let commit = output("git", &["rev-parse", "HEAD"], repository)?;
    let mut arguments = vec![
        "status".to_owned(),
        "--porcelain".to_owned(),
        "--".to_owned(),
        ".".to_owned(),
    ];
    if let Some(relative) = excluded.and_then(|folder| folder.strip_prefix(repository).ok()) {
        arguments.push(format!(":(exclude){}", relative.display()));
    }
    let status = output(
        "git",
        &arguments.iter().map(String::as_str).collect::<Vec<_>>(),
        repository,
    )?;
    let dirty = if status.trim().is_empty() {
        ""
    } else {
        "-dirty"
    };
    Ok((commit.trim().to_owned(), dirty))
}

fn stage_build(
    root: &Path,
    engine: &Path,
    release: bool,
    app: &Path,
    folder: &Path,
    verbose: bool,
) -> Result<(), String> {
    invoke(
        "cmake",
        &[
            &format!("-DENGINE={}", path(engine)),
            &format!("-DPROFILE={}", if release { "Release" } else { "Debug" }),
            &format!("-DAPP={}", path(app)),
            &format!("-DSTAGE={}", path(folder)),
            "-P",
            &path(&root.join("packaging/linux/stage.cmake")),
        ],
        root,
        verbose,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };

    use super::package;
    use crate::commands::build::{
        HELPER_PROCESS_TARGETS, app_build_stamp, engine_fingerprint, photon_executable,
    };

    const ENGINE: &str = "build/engine-release";
    const VCPKG: &str = "build/engine-release/vcpkg_installed/x64-linux-dynamic";
    const VCPKG_LIBRARY: &str = "int fixture_value(void) { return 2; }";
    const EMBEDDER: &str =
        "int fixture_value(void); int engine_value(void) { return fixture_value() + 1; }";
    const APP: &str =
        "int engine_value(void); int main(void) { return engine_value() == 3 ? 0 : 1; }";
    const HELPER: &str =
        "int fixture_value(void); int main(void) { return fixture_value() == 2 ? 0 : 1; }";

    struct Fixture {
        root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("photon-cli-package-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let fixture = Self { root };
            let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            for file in [
                "LICENSE",
                "packaging/linux/stage.cmake",
                "Engine/LICENSE",
                "Engine/Meta/CMake/ladybird_helper_processes.cmake",
                "vendor/gpui-ce/LICENSE.md",
            ] {
                let contents = std::fs::read(checkout.join(file)).unwrap();
                fixture.write(file, &contents);
            }
            fixture.write(".gitignore", b"build/\ntarget/\nEngine/\nvendor/\n");
            for repository in ["Engine", "vendor/gpui-ce", "."] {
                fixture.git(repository, &["init", "-q"]);
                fixture.git(repository, &["add", "-A"]);
                fixture.git(repository, &["commit", "-qm", "fixture"]);
            }

            fixture.write(&format!("{ENGINE}/share/Lagom/fonts/fixture.ttf"), b"font");
            fixture.write(&format!("{VCPKG}/share/zlib/copyright"), b"notice");
            fixture.write_cache("Release");
            fixture.compile(
                VCPKG_LIBRARY,
                &format!("{VCPKG}/lib/libfixture.so.1.0"),
                &[
                    "-shared",
                    "-Wl,-soname,libfixture.so.1",
                    "-Wl,--enable-new-dtags,-rpath,$ORIGIN",
                ],
            );
            symlink(
                "libfixture.so.1.0",
                fixture.path(&format!("{VCPKG}/lib/libfixture.so.1")),
            )
            .unwrap();
            fixture.compile(
                EMBEDDER,
                &format!("{ENGINE}/lib64/liblagom-photonembedder.so"),
                &[
                    "-shared",
                    &fixture.library_link_flags(&format!("{VCPKG}/lib")),
                    "-l:libfixture.so.1",
                ],
            );
            fixture.link_app(&[
                &fixture.library_link_flags(&format!("{ENGINE}/lib64")),
                "-llagom-photonembedder",
            ]);
            fixture.link_helper(&[&fixture.library_link_flags(&format!("{VCPKG}/lib"))]);
            std::fs::write(
                fixture.path(&format!("{ENGINE}/.photon-build-fingerprint")),
                engine_fingerprint(&fixture.root, true).unwrap(),
            )
            .unwrap();
            std::fs::write(
                photon_executable(&fixture.root, true).with_extension("photon-build-stamp"),
                app_build_stamp(&fixture.root, true).unwrap(),
            )
            .unwrap();
            fixture
        }

        fn package(&self) -> Result<PathBuf, String> {
            package(&self.root, true, &self.path("dist"), false)
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn write(&self, relative: &str, contents: &[u8]) {
            let path = self.path(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }

        fn write_cache(&self, build_type: &str) {
            let engine = self.path(ENGINE);
            let cache = format!(
                "CMAKE_BUILD_TYPE:STRING={build_type}\n\
                 CMAKE_HOME_DIRECTORY:INTERNAL={}\n\
                 CMAKE_INSTALL_LIBDIR:PATH=lib64\n\
                 PHOTON_HELPER_PROCESS_DIR:UNINITIALIZED={}\n\
                 VCPKG_INSTALLED_DIR:PATH={}\n\
                 VCPKG_TARGET_TRIPLET:STRING=x64-linux-dynamic\n",
                self.path("Engine").display(),
                engine.join("bin").display(),
                engine.join("vcpkg_installed").display(),
            );
            self.write(&format!("{ENGINE}/CMakeCache.txt"), cache.as_bytes());
        }

        fn library_link_flags(&self, relative: &str) -> String {
            let directory = self.path(relative).display().to_string();
            format!("-Wl,--enable-new-dtags,-L,{directory},-rpath,{directory}")
        }

        fn link_app(&self, arguments: &[&str]) {
            let app = photon_executable(&self.root, true);
            let app = app.strip_prefix(&self.root).unwrap().to_str().unwrap();
            self.compile(APP, app, arguments);
        }

        fn link_helper(&self, arguments: &[&str]) {
            let first = format!("{ENGINE}/bin/{}", HELPER_PROCESS_TARGETS[0]);
            let mut arguments = arguments.to_vec();
            arguments.push("-l:libfixture.so.1");
            self.compile(HELPER, &first, &arguments);
            for helper in &HELPER_PROCESS_TARGETS[1..] {
                std::fs::copy(
                    self.path(&first),
                    self.path(&format!("{ENGINE}/bin/{helper}")),
                )
                .unwrap();
            }
        }

        fn compile(&self, source: &str, output: &str, arguments: &[&str]) {
            let output = self.path(output);
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            let mut compiler = Command::new("cc")
                .args(["-fPIC", "-x", "c", "-", "-o"])
                .arg(&output)
                .args(arguments)
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            compiler
                .stdin
                .take()
                .unwrap()
                .write_all(source.as_bytes())
                .unwrap();
            assert!(
                compiler.wait().unwrap().success(),
                "cannot compile {}",
                output.display()
            );
            if output == photon_executable(&self.root, true) {
                std::fs::write(
                    output.with_extension("photon-build-stamp"),
                    app_build_stamp(&self.root, true).unwrap(),
                )
                .unwrap();
            }
        }

        fn git(&self, repository: &str, arguments: &[&str]) {
            let status = Command::new("git")
                .args([
                    "-c",
                    "user.name=Photon Test",
                    "-c",
                    "user.email=photon-test@example.invalid",
                ])
                .args(arguments)
                .current_dir(self.path(repository))
                .status()
                .unwrap();
            assert!(status.success(), "git {arguments:?} failed");
        }
    }

    #[test]
    fn package_runs_from_another_folder_without_the_checkout() {
        let fixture = Fixture::new("relocated");
        let archive = fixture.package().unwrap();
        let output = archive.parent().unwrap();
        let archive_name = archive.file_name().unwrap().to_str().unwrap();
        let name = archive_name.strip_suffix(".tar.xz").unwrap();
        let mut files = std::fs::read_dir(output)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(
            files,
            [archive_name.to_owned(), format!("{archive_name}.sha256")]
        );
        assert!(fixture.package().unwrap_err().contains("already exists"));
        let checksum = Command::new("sha256sum")
            .args(["--check", &format!("{archive_name}.sha256")])
            .current_dir(output)
            .output()
            .unwrap();
        assert!(checksum.status.success());

        let extracted = fixture.path("another place");
        std::fs::create_dir(&extracted).unwrap();
        let unpacked = Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&extracted)
            .status()
            .unwrap();
        assert!(unpacked.success());
        let app = photon_executable(&fixture.root, true);
        let app_name = app.file_name().unwrap().to_str().unwrap().to_owned();
        std::fs::remove_dir_all(fixture.path("build")).unwrap();
        std::fs::remove_dir_all(fixture.path("target")).unwrap();

        let package = extracted.join(name);
        for executable in
            std::iter::once(app_name.as_str()).chain(HELPER_PROCESS_TARGETS.iter().copied())
        {
            let status = Command::new(package.join("bin").join(executable))
                .current_dir("/")
                .env_remove("LD_LIBRARY_PATH")
                .status()
                .unwrap();
            assert!(
                status.success(),
                "{executable} did not run from the extracted package"
            );
        }
        let version = std::fs::read_to_string(package.join("VERSION")).unwrap();
        assert!(version.contains("\nprofile=release\n"), "{version}");
        let host = std::fs::read_to_string(package.join("HOST-LIBRARIES")).unwrap();
        assert!(host.lines().any(|line| line == "libc.so.6"), "{host}");
        assert!(!host.contains("libfixture"), "{host}");
        for file in [
            "LICENSE",
            "README",
            "lib/libfixture.so.1",
            "share/Lagom/fonts/fixture.ttf",
            "share/licenses/Engine/LICENSE",
            "share/licenses/gpui-ce/LICENSE.md",
            "share/licenses/vcpkg/zlib/copyright",
        ] {
            assert!(
                package.join(file).exists(),
                "{file} is missing from the package"
            );
        }
    }

    #[test]
    fn package_debug_build_uses_debug_vcpkg_libraries() {
        let fixture = Fixture::new("debug");
        let engine = "build/engine-debug";
        let libraries = format!("{engine}/vcpkg_installed/x64-linux-dynamic/debug/lib");
        fixture.compile(
            VCPKG_LIBRARY,
            &format!("{libraries}/libfixture.so.1"),
            &[
                "-shared",
                "-Wl,-soname,libfixture.so.1",
                "-Wl,-rpath,$ORIGIN",
            ],
        );
        fixture.compile(
            EMBEDDER,
            &format!("{engine}/lib64/liblagom-photonembedder.so"),
            &[
                "-shared",
                &fixture.library_link_flags(&libraries),
                "-l:libfixture.so.1",
            ],
        );
        for helper in HELPER_PROCESS_TARGETS {
            fixture.compile(
                HELPER,
                &format!("{engine}/bin/{helper}"),
                &[
                    &fixture.library_link_flags(&libraries),
                    "-l:libfixture.so.1",
                ],
            );
        }
        fixture.compile(
            APP,
            "target/debug/Photon",
            &[
                &fixture.library_link_flags(&format!("{engine}/lib64")),
                "-llagom-photonembedder",
            ],
        );
        let cache = std::fs::read_to_string(fixture.path(&format!("{ENGINE}/CMakeCache.txt")))
            .unwrap()
            .replace(
                "CMAKE_BUILD_TYPE:STRING=Release",
                "CMAKE_BUILD_TYPE:STRING=Debug",
            )
            .replace("engine-release", "engine-debug");
        fixture.write(&format!("{engine}/CMakeCache.txt"), cache.as_bytes());
        fixture.write(&format!("{engine}/share/Lagom/fonts/fixture.ttf"), b"font");
        fixture.write(
            &format!("{engine}/.photon-build-fingerprint"),
            engine_fingerprint(&fixture.root, false).unwrap().as_bytes(),
        );
        std::fs::write(
            photon_executable(&fixture.root, false).with_extension("photon-build-stamp"),
            app_build_stamp(&fixture.root, false).unwrap(),
        )
        .unwrap();
        let archive = package(&fixture.root, false, &fixture.path("dist"), false).unwrap();
        assert!(
            archive
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("-debug.tar.xz")
        );
        let unpacked = fixture.path("relocated debug");
        std::fs::create_dir(&unpacked).unwrap();
        assert!(
            Command::new("tar")
                .arg("-xf")
                .arg(&archive)
                .arg("-C")
                .arg(&unpacked)
                .status()
                .unwrap()
                .success()
        );
        std::fs::remove_dir_all(fixture.path("build")).unwrap();
        let folder = std::fs::read_dir(&unpacked)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(
            Command::new(folder.join("bin/Photon"))
                .current_dir("/")
                .env_remove("LD_LIBRARY_PATH")
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn package_rejects_stale_application_provenance() {
        let fixture = Fixture::new("stale-application");
        let app = photon_executable(&fixture.root, true);
        let stamp = app.with_extension("photon-build-stamp");
        std::fs::remove_file(&stamp).unwrap();
        assert!(fixture.package().unwrap_err().contains("provenance"));
        std::fs::write(&stamp, app_build_stamp(&fixture.root, true).unwrap()).unwrap();
        fixture.write("Cargo.toml", b"# newer application sources");
        assert!(fixture.package().unwrap_err().contains("provenance"));
        fixture.git(".", &["add", "Cargo.toml"]);
        fixture.git(".", &["commit", "-qm", "new application sources"]);
        assert!(fixture.package().unwrap_err().contains("provenance"));
        std::fs::write(&stamp, app_build_stamp(&fixture.root, true).unwrap()).unwrap();
        let mut binary = std::fs::read(&app).unwrap();
        binary.push(0);
        std::fs::write(&app, binary).unwrap();
        assert!(fixture.package().unwrap_err().contains("provenance"));
    }

    #[test]
    fn package_prefers_the_bundled_copy_of_a_host_transitive_dependency() {
        let fixture = Fixture::new("host-dependency");
        let dbus = Path::new("/lib/x86_64-linux-gnu/libdbus-1.so.3");
        std::fs::copy(dbus, fixture.path(&format!("{VCPKG}/lib/libdbus-1.so.3"))).unwrap();
        fixture.compile(
            EMBEDDER,
            &format!("{ENGINE}/lib64/liblagom-photonembedder.so"),
            &[
                "-shared",
                &fixture.library_link_flags(&format!("{VCPKG}/lib")),
                "-l:libfixture.so.1",
                "-Wl,--no-as-needed",
                "-l:libdbus-1.so.3",
            ],
        );
        fixture.compile(
            HELPER,
            &format!("{ENGINE}/bin/AudioServer"),
            &[
                &fixture.library_link_flags(&format!("{VCPKG}/lib")),
                "-l:libfixture.so.1",
                "-Wl,--no-as-needed",
                "-l:libpulse.so.0",
            ],
        );
        assert!(fixture.package().unwrap().is_file());
    }

    #[test]
    fn package_preserves_an_existing_staging_directory() {
        let fixture = Fixture::new("existing-stage");
        let archive = fixture.package().unwrap();
        let filename = archive.file_name().unwrap().to_str().unwrap();
        let name = filename.strip_suffix(".tar.xz").unwrap();
        let existing = fixture.path(&format!("dist/.{name}.{}", std::process::id()));
        std::fs::create_dir(&existing).unwrap();
        let marker = existing.join("keep");
        std::fs::write(&marker, b"existing data").unwrap();
        std::fs::remove_file(archive.with_file_name(format!("{filename}.sha256"))).unwrap();
        std::fs::remove_file(&archive).unwrap();
        let error = fixture.package().unwrap_err();
        assert!(error.contains("cannot create"), "{error}");
        assert!(
            marker.exists(),
            "a failed run must not delete a folder it did not create"
        );
    }

    #[test]
    fn package_requires_a_completed_build() {
        let fixture = Fixture::new("incomplete");
        std::fs::remove_file(fixture.path(&format!("{ENGINE}/.photon-build-fingerprint"))).unwrap();
        assert!(fixture.package().unwrap_err().contains("incomplete"));
        std::fs::remove_file(photon_executable(&fixture.root, true)).unwrap();
        let error = fixture.package().unwrap_err();
        assert!(
            error.contains("missing") && error.contains("photon build"),
            "{error}"
        );
        assert!(!fixture.path("dist").exists());
    }

    #[test]
    fn package_requires_every_engine_helper() {
        let fixture = Fixture::new("missing-helper");
        std::fs::remove_file(fixture.path(&format!("{ENGINE}/bin/WebRTCClient"))).unwrap();
        let error = fixture.package().unwrap_err();
        assert!(error.contains("WebRTCClient"), "{error}");
        assert_eq!(std::fs::read_dir(fixture.path("dist")).unwrap().count(), 0);
    }

    #[test]
    fn package_refuses_to_mix_profiles() {
        let fixture = Fixture::new("mixed-profile");
        fixture.write_cache("Debug");
        let error = fixture.package().unwrap_err();
        assert!(error.contains("refusing to mix profiles"), "{error}");

        fixture.write_cache("Release");
        fixture.compile(
            EMBEDDER,
            "build/engine-debug/lib64/liblagom-photonembedder.so",
            &[
                "-shared",
                &fixture.library_link_flags(&format!("{VCPKG}/lib")),
                "-l:libfixture.so.1",
            ],
        );
        fixture.link_app(&[
            &fixture.library_link_flags("build/engine-debug/lib64"),
            "-llagom-photonembedder",
        ]);
        let error = fixture.package().unwrap_err();
        assert!(error.contains("another profile"), "{error}");
    }

    #[test]
    fn package_refuses_unresolved_and_colliding_libraries() {
        let fixture = Fixture::new("libraries");
        fixture.compile(
            "int fixture_value(void) { return 5; }",
            &format!("{ENGINE}/lib64/libfixture.so.1"),
            &["-shared", "-Wl,-soname,libfixture.so.1"],
        );
        fixture.link_app(&[
            &fixture.library_link_flags(&format!("{ENGINE}/lib64")),
            "-llagom-photonembedder",
            "-Wl,--no-as-needed",
            "-l:libfixture.so.1",
        ]);
        let error = fixture.package().unwrap_err();
        assert!(error.contains("libfixture.so.1"), "{error}");

        fixture.link_app(&[
            &fixture.library_link_flags(&format!("{ENGINE}/lib64")),
            "-llagom-photonembedder",
        ]);
        std::fs::remove_file(fixture.path(&format!("{ENGINE}/lib64/libfixture.so.1"))).unwrap();
        for library in ["libfixture.so.1", "libfixture.so.1.0"] {
            std::fs::remove_file(fixture.path(&format!("{VCPKG}/lib/{library}"))).unwrap();
        }
        let error = fixture.package().unwrap_err();
        assert!(error.contains("unresolved libraries"), "{error}");
    }

    #[test]
    fn package_refuses_files_it_cannot_relocate() {
        let fixture = Fixture::new("relocation");
        let mut library =
            std::fs::read(fixture.path(&format!("{VCPKG}/lib/libfixture.so.1.0"))).unwrap();
        library[18..20].copy_from_slice(&183u16.to_le_bytes());
        fixture.write(&format!("{VCPKG}/lib/libfixture-arm64.so"), &library);
        let error = fixture.package().unwrap_err();
        assert!(error.contains("libfixture-arm64.so"), "{error}");
        std::fs::remove_file(fixture.path(&format!("{VCPKG}/lib/libfixture-arm64.so"))).unwrap();

        fixture.link_helper(&[&format!(
            "-Wl,-L,{}",
            fixture.path(&format!("{VCPKG}/lib")).display()
        )]);
        let error = fixture.package().unwrap_err();
        assert!(error.contains("has no RUNPATH entry"), "{error}");

        fixture.link_helper(&[&fixture.library_link_flags(&format!("{VCPKG}/lib"))]);
        fixture.compile(
            "int main(void) { return 0; }",
            photon_executable(&fixture.root, true)
                .strip_prefix(&fixture.root)
                .unwrap()
                .to_str()
                .unwrap(),
            &[],
        );
        let error = fixture.package().unwrap_err();
        assert!(error.contains("UI-only"), "{error}");
    }
}
