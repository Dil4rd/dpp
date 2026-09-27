use super::*;

#[test]
fn tag_order_ranks_by_version_not_text() {
    assert!(tag_order("v7.0.1") > tag_order("v2.8.0"));
    assert!(tag_order("v1.10.0") > tag_order("v1.9.0"));
    assert_eq!(tag_order("v4"), Some((4, 0, 0)));
}

#[test]
fn tag_order_rejects_non_versions() {
    // A moving tag or a branch-shaped release is not comparable.
    assert_eq!(tag_order("release/v1"), None);
    assert_eq!(tag_order("nightly"), None);
    assert_eq!(tag_order("v1.2.3.4"), None);
}

#[test]
fn normalise_lock_drops_only_member_checksums() {
    let lock = "\
[[package]]
name = \"cmpfs\"
version = \"0.1.0\"
checksum = \"aaaa\"

[[package]]
name = \"byteorder\"
version = \"1.5.0\"
checksum = \"bbbb\"
";
    let out = normalise_lock(lock, &["cmpfs".to_owned()]);
    assert!(!out.contains("aaaa"), "member checksum should be dropped");
    assert!(out.contains("bbbb"), "registry checksum must be kept");
    assert!(
        out.contains("name = \"cmpfs\""),
        "the member itself stays pinned by version"
    );
}
