//! Path confinement, byte typing and the re-check before storing (ops-cover.v2).
use super::{
    CoverImportRequest, CoverSource, OpsImportError, PlannedCover, image_type, read_cover,
    relative_media_path, sha256_hex, store_file,
};
use std::path::PathBuf;
use uuid::Uuid;

const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPVP8 unit";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ops-cover-unit-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
#[test]
fn a_link_on_any_step_of_the_way_is_refused() {
    let (covers, outside) = (scratch("covers"), scratch("outside"));
    std::fs::create_dir_all(covers.join("ws/media")).unwrap();
    std::fs::write(covers.join("ws/media/real.webp"), WEBP).unwrap();
    std::fs::write(covers.join("ws/media/empty.webp"), b"").unwrap();
    std::fs::write(outside.join("elsewhere.webp"), WEBP).unwrap();
    std::os::unix::fs::symlink(&outside, covers.join("ws/linked")).unwrap();
    std::os::unix::fs::symlink(
        outside.join("elsewhere.webp"),
        covers.join("ws/media/link.webp"),
    )
    .unwrap();
    let read = |relative: &str| read_cover(&covers, relative).unwrap();
    assert_eq!(
        read("ws/media/real.webp"),
        CoverSource::Bytes(WEBP.to_vec())
    );
    assert_eq!(
        read("ws/linked/elsewhere.webp"),
        CoverSource::Refused,
        "a linked folder would leave the copied tree"
    );
    assert_eq!(read("ws/media/link.webp"), CoverSource::Refused);
    assert_eq!(read("ws/media/empty.webp"), CoverSource::Refused);
    assert_eq!(read("ws/media/absent.webp"), CoverSource::Missing);
    assert_eq!(read("ws/absent/x.webp"), CoverSource::Missing);
    let _ = (
        std::fs::remove_dir_all(covers),
        std::fs::remove_dir_all(outside),
    );
}

#[test]
fn a_file_changed_after_the_preview_is_not_stored() {
    let (covers, media) = (scratch("covers"), scratch("media"));
    let request = CoverImportRequest {
        publications_export: Vec::new(),
        covers_dir: covers.clone(),
        media_root: media.clone(),
    };
    let planned = PlannedCover {
        publication_ref: Uuid::new_v4(),
        source_path: "one.webp".into(),
        sha256: sha256_hex(WEBP),
        byte_size: i64::try_from(WEBP.len()).unwrap(),
        mime_type: "image/webp",
    };
    // Same size, different last byte: only the hash tells them apart.
    let mut changed = WEBP.to_vec();
    *changed.last_mut().unwrap() = b'X';
    std::fs::write(covers.join("one.webp"), &changed).unwrap();
    assert!(matches!(
        store_file(&request, &planned),
        Err(OpsImportError::CoverFileChanged)
    ));
    assert!(
        !media.join(planned.storage_key()).exists(),
        "nothing is stored under the planned name"
    );
    std::fs::remove_file(covers.join("one.webp")).unwrap();
    assert!(matches!(
        store_file(&request, &planned),
        Err(OpsImportError::CoverFileChanged)
    ));
    std::fs::write(covers.join("one.webp"), WEBP).unwrap();
    assert!(matches!(store_file(&request, &planned), Ok(true)));
    assert_eq!(
        std::fs::read(media.join(planned.storage_key())).unwrap(),
        WEBP
    );
    let _ = (
        std::fs::remove_dir_all(covers),
        std::fs::remove_dir_all(media),
    );
}

#[test]
fn only_workbench_media_paths_inside_the_folder_are_accepted() {
    assert_eq!(
        relative_media_path("/api/local-media/workspaces/w1/media/covers/a%20b.webp").as_deref(),
        Some("workspaces/w1/media/covers/a b.webp")
    );
    for refused in [
        "/api/local-media/../etc/passwd",
        "/api/local-media/workspaces/../../x.webp",
        "/api/local-media/workspaces//x.webp",
        "/api/local-media/%2E%2E/x.webp",
        "/api/local-media/a%5Cb.webp",
        "https://example.com/x.webp",
        "/api/local-media/",
        "/api/local-media/%ZZ.webp",
    ] {
        assert!(relative_media_path(refused).is_none(), "{refused}");
    }
}

#[test]
fn the_bytes_decide_the_image_type() {
    assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
    assert_eq!(image_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
    assert_eq!(
        image_type(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
        Some("image/png")
    );
    assert_eq!(image_type(b"<svg onload=alert(1)>"), None);
    assert_eq!(image_type(b"RIFF\0\0\0\0WAVE"), None);
}
