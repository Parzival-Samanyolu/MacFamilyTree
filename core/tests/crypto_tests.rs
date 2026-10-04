use kintree_core::crypto::*;

#[test]
fn roundtrip_with_real_parameters() {
    let blob = encrypt(b"secret family data", "correct horse").unwrap();
    assert!(is_encrypted(&blob));
    assert!(!blob.windows(6).any(|w| w == b"secret"));
    assert_eq!(
        decrypt(&blob, "correct horse").unwrap(),
        b"secret family data"
    );
}

#[test]
fn wrong_password_tampering_and_garbage_are_rejected() {
    let blob = encrypt_fast_for_tests(b"data", "pw").unwrap();
    assert_eq!(
        decrypt(&blob, "other"),
        Err(CryptoError::WrongPasswordOrCorrupt)
    );
    assert_eq!(decrypt(&blob, ""), Err(CryptoError::EmptyPassword));
    for i in [0usize, 6, 7, 30, blob.len() - 1] {
        let mut t = blob.clone();
        t[i] ^= 1;
        assert!(decrypt(&t, "pw").is_err(), "flip at {i}");
    }
    assert_eq!(
        decrypt(b"plain text file", "pw"),
        Err(CryptoError::NotEncrypted)
    );
    assert_eq!(decrypt(&blob[..10], "pw"), Err(CryptoError::NotEncrypted));
    assert_eq!(encrypt(b"x", ""), Err(CryptoError::EmptyPassword));
}

#[test]
fn every_encryption_is_unique_and_large_inputs_work() {
    let big = vec![7u8; 300_000];
    let a = encrypt_fast_for_tests(&big, "pw").unwrap();
    let b = encrypt_fast_for_tests(&big, "pw").unwrap();
    assert_ne!(a, b, "random salt and nonce");
    assert_eq!(decrypt(&a, "pw").unwrap(), big);
    assert_eq!(
        decrypt(&encrypt_fast_for_tests(b"", "pw").unwrap(), "pw").unwrap(),
        b""
    );
}

#[test]
fn an_encrypted_project_image_restores_everything() {
    use kintree_core::{gedcom, media, store::Store};
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n0 TRLR\n",
    )
    .unwrap();
    media::import(&mut s, "note.txt", b"hello", None).unwrap();
    let blob = encrypt_fast_for_tests(&s.to_bytes().unwrap(), "pw").unwrap();
    let back = Store::from_bytes(&decrypt(&blob, "pw").unwrap()).unwrap();
    assert_eq!(back.rows("person").unwrap().len(), 1);
    assert_eq!(
        back.search_persons("kaya", 5).unwrap().len(),
        1,
        "search index survives"
    );
    let id = back.rows("media").unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(media::file(&back, &id).unwrap().unwrap().2, b"hello");
    assert!(Store::from_bytes(b"garbage").is_err());
}
