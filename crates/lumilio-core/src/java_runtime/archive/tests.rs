use std::io::Write;

use super::*;

fn tar_gz(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    let bytes = builder.into_inner().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&bytes).unwrap();
    encoder.finish().unwrap()
}

fn file(builder: &mut tar::Builder<Vec<u8>>, path: &str, body: &[u8], mode: u32) {
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(mode);
    header.set_entry_type(tar::EntryType::Regular);
    builder.append_data(&mut header, path, body).unwrap();
}

fn link(builder: &mut tar::Builder<Vec<u8>>, path: &str, target: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    builder.append_link(&mut header, path, target).unwrap();
}

#[test]
fn a_mac_archive_drops_its_top_folder_and_keeps_links_and_modes() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("jdk.tar.gz");
    fs::write(
        &archive,
        tar_gz(|builder| {
            let top = "graalvm-jdk-21.0.8+12.1/Contents";
            file(
                builder,
                &format!("{top}/Home/release"),
                b"JAVA_VERSION=\"21.0.8\"",
                0o644,
            );
            file(builder, &format!("{top}/Home/bin/java"), b"#!", 0o755);
            link(
                builder,
                &format!("{top}/MacOS/libjli.dylib"),
                "../Home/lib/libjli.dylib",
            );
        }),
    )
    .unwrap();
    let into = dir.path().join("graalvm-21");
    unpack(&archive, ArchiveKind::TarGz, &into).unwrap();
    assert!(into.join("Contents/Home/release").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(into.join("Contents/Home/bin/java"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        assert_eq!(
            fs::read_link(into.join("Contents/MacOS/libjli.dylib")).unwrap(),
            Path::new("../Home/lib/libjli.dylib")
        );
    }
}

#[test]
fn links_out_of_the_runtime_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for target in ["../../../etc/passwd", "/etc/passwd"] {
        let archive = dir.path().join("bad.tar.gz");
        fs::write(
            &archive,
            tar_gz(|builder| link(builder, "jdk/bin/escape", target)),
        )
        .unwrap();
        let error = unpack(&archive, ArchiveKind::TarGz, &dir.path().join("out")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{target}");
    }
}

#[test]
fn entries_climbing_out_are_refused() {
    assert!(inner_path(Path::new("jdk/../../x")).is_err());
    assert!(inner_path(Path::new("/jdk/bin")).is_err());
    assert_eq!(inner_path(Path::new("jdk")).unwrap(), None);
    assert_eq!(
        inner_path(Path::new("./jdk/bin/java")).unwrap(),
        Some(PathBuf::from("bin/java"))
    );
}

#[test]
fn a_windows_archive_unpacks_the_same_way() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("jdk.zip");
    {
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer
            .add_directory("graalvm-jdk-25/bin/", options)
            .unwrap();
        writer
            .start_file("graalvm-jdk-25/release", options)
            .unwrap();
        writer.write_all(b"JAVA_VERSION=\"25\"").unwrap();
        writer
            .start_file("graalvm-jdk-25/bin/java.exe", options)
            .unwrap();
        writer.write_all(b"MZ").unwrap();
        writer.finish().unwrap();
    }
    let into = dir.path().join("graalvm-25");
    unpack(&archive, ArchiveKind::Zip, &into).unwrap();
    assert_eq!(fs::read(into.join("bin/java.exe")).unwrap(), b"MZ");
    assert!(into.join("release").is_file());
}
