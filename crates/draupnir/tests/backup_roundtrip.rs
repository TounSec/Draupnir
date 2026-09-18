use std::fs;
use std::io::BufReader;
use std::path::Path;

fn build_archive(src: &Path, out: &Path, recipient: &dyn age::Recipient) {
    let file = fs::File::create(out).unwrap();
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(recipient))
            .expect("creating encryptor");
    let age_w = encryptor.wrap_output(file).expect("writing age header");
    let zstd_w = zstd::stream::write::Encoder::new(age_w, 3)
        .expect("initialising zstd encoder");
    let mut tar_b = tar::Builder::new(zstd_w);
    tar_b.follow_symlinks(false);

    let mut f = fs::File::open(src.join("hello.txt")).unwrap();
    tar_b.append_file("hello.txt", &mut f).unwrap();

    tar_b
        .append_dir("subdir", src.join("subdir"))
        .unwrap();
    let mut f = fs::File::open(src.join("subdir/nested.txt")).unwrap();
    tar_b.append_file("subdir/nested.txt", &mut f).unwrap();

    let zstd_w = tar_b.into_inner().expect("finalising tar");
    let age_w = zstd_w.finish().expect("finalising zstd");
    let file = age_w.finish().expect("finalising age");
    file.sync_all().expect("syncing archive");
}

#[test]
fn roundtrip_encrypt_compress_archive() {
    let tmp = tempfile::tempdir().unwrap();

    // Source tree
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("subdir")).unwrap();
    fs::write(src.join("hello.txt"), b"hello world\n").unwrap();
    fs::write(src.join("subdir/nested.txt"), b"nested content\n").unwrap();

    // Generate ephemeral age keypair
    let identity = age::x25519::Identity::generate();
    let recipient = identity.to_public();

    // Build archive
    let archive = tmp.path().join("test.tar.zst.age");
    build_archive(&src, &archive, &recipient);

    // Decrypt and verify contents
    let encrypted = BufReader::new(fs::File::open(&archive).unwrap());
    let decryptor = age::Decryptor::new(encrypted).expect("parsing age header");
    let decrypted = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .expect("decrypting archive");
    let zstd_r = zstd::stream::read::Decoder::new(decrypted)
        .expect("initialising zstd decoder");
    let mut tar_a = tar::Archive::new(zstd_r);

    let mut entries: Vec<String> = tar_a
        .entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
        .collect();
    entries.sort();

    assert!(
        entries.iter().any(|p| p == "hello.txt"),
        "hello.txt missing from archive: {entries:?}"
    );
    assert!(
        entries.iter().any(|p| p == "subdir/nested.txt"),
        "subdir/nested.txt missing from archive: {entries:?}"
    );
}
