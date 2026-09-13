//! Unit tests for the parent module, split out of `toc.rs` for size.

use super::*;

#[test]
fn parses_link_target_for_symlink_entry() {
    // Matches the real element order produced by macOS `xar`: <link> comes
    // before <type>/<name>.
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<link type="file">../README.txt</link>
<type>symlink</type>
<name>readme-link.txt</name>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_type, XarFileType::Symlink);
    assert_eq!(files[0].name, "readme-link.txt");
    assert_eq!(files[0].link.as_deref(), Some("../README.txt"));
}

#[test]
fn non_symlink_entries_have_no_link_target() {
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<name>plain.txt</name>
<type>file</type>
<link type="file">must-not-leak</link>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files[0].link, None);
}

#[test]
fn ea_block_name_before_real_name_does_not_clobber_it() {
    // Real XAR TOCs (per macOS `xar`) emit <ea> — which carries its own
    // <name>, e.g. "com.apple.provenance" — before the file's real <type>/<name>.
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<ea id="0">
  <length>19</length>
  <offset>20</offset>
  <size>11</size>
  <name>com.apple.provenance</name>
</ea>
<type>file</type>
<name>real-name.txt</name>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files[0].name, "real-name.txt");
    assert_eq!(files[0].path, "real-name.txt");
}

#[test]
fn ea_block_name_after_real_name_does_not_clobber_it() {
    // The inverse ordering: if a producer emits <ea> AFTER <type>/<name>
    // instead of before, the pre-fix parser (which tracked `current_tag`
    // for any tag anywhere inside <file>, regardless of nesting depth)
    // would let <ea>'s <name> overwrite the file's real name here, since
    // it was the last "name" tag seen. This must not happen.
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<type>file</type>
<name>real-name.txt</name>
<ea id="0">
  <length>19</length>
  <offset>20</offset>
  <size>11</size>
  <name>com.apple.provenance</name>
</ea>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files[0].name, "real-name.txt");
    assert_eq!(files[0].path, "real-name.txt");
}

#[test]
fn ea_offset_length_size_do_not_leak_into_data_descriptor() {
    // <ea> carries its own <offset>/<length>/<size>, sitting alongside a
    // real <data> block with different values. The two must not mix.
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<ea id="0">
  <length>19</length>
  <offset>999</offset>
  <size>11</size>
</ea>
<type>file</type>
<name>payload.bin</name>
<data>
  <offset>0</offset>
  <length>42</length>
  <size>42</size>
  <encoding style="application/octet-stream"/>
</data>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    let data = files[0].data.as_ref().expect("data descriptor present");
    assert_eq!(data.offset, 0);
    assert_eq!(data.length, 42);
    assert_eq!(data.size, 42);
}

#[test]
fn nested_directory_and_symlink_link_targets_are_scoped_per_file() {
    // A directory containing a symlink; the directory's own <name> must
    // not bleed into the child's fields or vice versa, and the full path
    // must be built from both ancestor names.
    let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
<type>directory</type>
<name>dir</name>
<file id="2">
  <link type="file">../target.txt</link>
  <type>symlink</type>
  <name>link.txt</name>
</file>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files.len(), 2);
    let link_entry = files.iter().find(|f| f.name == "link.txt").unwrap();
    assert_eq!(link_entry.path, "dir/link.txt");
    assert_eq!(link_entry.link.as_deref(), Some("../target.txt"));
    let dir_entry = files.iter().find(|f| f.name == "dir").unwrap();
    assert_eq!(dir_entry.link, None);
}

#[test]
fn preserves_exact_link_text_across_xml_event_types() {
    let cases: &[(&[u8], &str)] = &[
        (
            br#"<xar><toc><file id="1"><link type="file"> target </link><type>symlink</type><name>link</name></file></toc></xar>"#,
            " target ",
        ),
        (
            br#"<xar><toc><file id="1"><link type="file"><![CDATA[../A&B]]></link><type>symlink</type><name>link</name></file></toc></xar>"#,
            "../A&B",
        ),
        (
            br#"<xar><toc><file id="1"><link type="file">foo<!-- split -->bar</link><type>symlink</type><name>link</name></file></toc></xar>"#,
            "foobar",
        ),
        (
            br#"<xar><toc><file id="1"><link type="file">A&amp;B</link><type>symlink</type><name>link</name></file></toc></xar>"#,
            "A&B",
        ),
    ];

    for &(xml, expected) in cases {
        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files[0].link.as_deref(), Some(expected));
    }
}

#[test]
fn nested_ea_data_does_not_overwrite_file_data() {
    let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data>
<offset>0</offset><length>4</length><size>4</size>
<encoding style="application/octet-stream"/>
<extracted-checksum>real-extracted</extracted-checksum>
<archived-checksum>real-archived</archived-checksum>
  </data>
  <ea id="0"><name>extension</name><data>
<offset>999</offset><length>3</length><size>3</size>
<encoding style="application/x-gzip"/>
<extracted-checksum>fake-extracted</extracted-checksum>
<archived-checksum>fake-archived</archived-checksum>
  </data></ea>
</file></toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    let data = files[0].data.as_ref().unwrap();
    assert_eq!((data.offset, data.length, data.size), (0, 4, 4));
    assert_eq!(data.encoding, "application/octet-stream");
    assert_eq!(data.extracted_checksum.as_deref(), Some("real-extracted"));
    assert_eq!(data.archived_checksum.as_deref(), Some("real-archived"));
}

#[test]
fn nested_ea_data_does_not_create_file_data() {
    let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>metadata-only</name>
  <ea id="0"><data><offset>9</offset><length>3</length><size>3</size></data></ea>
</file></toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert!(files[0].data.is_none());
}

#[test]
fn nested_data_does_not_interrupt_the_direct_file_data_block() {
    let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data>
<offset>0</offset>
<extension><data><offset>999</offset><length>3</length><size>3</size></data></extension>
<length>4</length><size>4</size>
<encoding style="application&#x2f;octet-stream"/>
  </data>
</file></toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    let data = files[0].data.as_ref().unwrap();
    assert_eq!((data.offset, data.length, data.size), (0, 4, 4));
    assert_eq!(data.encoding, "application/octet-stream");
}

#[test]
fn invalid_xml_text_is_reported_instead_of_silently_erased() {
    let xml = br#"<xar><toc><file id="1"><link>&unknown;</link><type>symlink</type><name>link</name></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn incomplete_direct_data_block_is_rejected() {
    let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data><offset>0</offset><length>4</length></data>
</file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn empty_direct_data_block_is_rejected() {
    let xml = br#"<xar><toc><file id="1"><type>file</type><name>payload</name><data/></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn nested_elements_inside_scalar_metadata_are_rejected() {
    let xml = br#"<xar><toc><file id="1"><type>symlink</type><name>link</name><link>before<extension/>after</link></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn duplicate_direct_data_blocks_are_rejected() {
    let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name><data></data><data></data>
</file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn paths_do_not_depend_on_parent_name_element_order() {
    let xml = br#"<xar><toc><file id="1"><type>directory</type>
  <file id="2"><type>directory</type>
<file id="3"><type>file</type><name>leaf</name></file><name>child</name>
  </file><name>parent</name>
</file></toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["parent/child/leaf", "parent/child", "parent"]
    );
}

#[test]
fn duplicate_scalar_fields_and_unknown_types_are_rejected() {
    let cases: &[&[u8]] = &[
        br#"<xar><toc><file><type>file</type><name>first</name><name>second</name></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><type>symlink</type><name>entry</name></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><offset>1</offset><length>0</length><size>0</size></data></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding style="a"/><encoding style="b"/></data></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding/></data></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><name/></file></toc></xar>"#,
        br#"<xar><toc><file><type>mystery</type><name>entry</name></file></toc></xar>"#,
    ];

    for xml in cases {
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }
}

#[test]
fn document_structure_and_required_symlink_state_are_rejected_when_malformed() {
    let cases: &[&[u8]] = &[
        br#"<toc><file><type>file</type><name>entry</name></file></toc>"#,
        br#"<xar></xar>"#,
        br#"<xar><toc/><toc/></xar>"#,
        br#"<xar><extension><toc/></extension></xar>"#,
        // `<xar>` has closed, so DocumentState is still SawXar here: only
        // the structural parent check rejects this.
        br#"<xar></xar><toc/>"#,
        br#"<xar><toc><file/></toc></xar>"#,
        br#"<xar><toc><file><name>entry</name></file></toc></xar>"#,
        br#"<xar><toc><file><type>symlink</type><name>link</name></file></toc></xar>"#,
        br#"<xar><toc><file><type>symlink</type><name>link</name><link/></file></toc></xar>"#,
        br#"<xar><toc/></xar><second/>"#,
    ];

    for xml in cases {
        assert!(
            matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))),
            "accepted malformed XML: {}",
            String::from_utf8_lossy(xml)
        );
    }
}

#[test]
fn duplicate_relevant_attributes_are_rejected() {
    let cases: &[&[u8]] = &[
        br#"<xar><toc><file id="1" id="2"><type>file</type><name>entry</name></file></toc></xar>"#,
        br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding style="a" style="b"/></data></file></toc></xar>"#,
    ];

    for xml in cases {
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }
}

#[test]
fn duplicate_attribute_on_a_read_name_is_rejected() {
    // A repeated attribute is not well-formed XML and must not be read as
    // though one of the two values had been chosen. quick-xml reports it
    // first; `attribute_value` is the backstop if that check is ever
    // relaxed, so assert the outcome rather than which layer produced it.
    let xml =
        br#"<xar><toc><file id="1" id="2"><type>file</type><name>a</name></file></toc></xar>"#;
    let Err(XarError::XmlParse(message)) = parse_toc_xml(xml) else {
        panic!("expected a parse error");
    };
    assert!(
        message.contains("duplicat"),
        "error should name the duplication, got {message:?}"
    );
}

#[test]
fn decodes_base64_encoded_name() {
    // Synthetic: no fixture in this repo exercises this. The only XAR in
    // the fixture set is kdk.dmg's KernelDebugKit.pkg, whose names are
    // all ASCII and so never carry enctype. This is the base64 encoding
    // of "こんにちは.txt".
    let xml = br#"<xar><toc>
  <file id="1">
<type>file</type>
<name enctype="base64">44GT44KT44Gr44Gh44GvLnR4dA==</name>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    assert_eq!(files[0].name, "こんにちは.txt");
    assert_eq!(files[0].path, "こんにちは.txt");
}

#[test]
fn base64_name_decoding_applies_within_nested_directories() {
    let xml = br#"<xar><toc>
  <file id="1">
<type>directory</type>
<name>unicode</name>
<file id="2">
  <type>file</type>
  <name enctype="base64">44GT44KT44Gr44Gh44GvLnR4dA==</name>
</file>
  </file>
</toc></xar>"#;

    let files = parse_toc_xml(xml).unwrap();
    let child = files.iter().find(|f| f.id == 2).unwrap();
    assert_eq!(child.name, "こんにちは.txt");
    assert_eq!(child.path, "unicode/こんにちは.txt");
}

#[test]
fn accepts_line_wrapped_base64_encoded_name() {
    const APPLE_XAR_BASE64_LINE_LENGTH: usize = 72;

    let name = "これはとても長いファイル名であり、XARのbase64ラッピングを検証します.txt";
    let encoded = base64::engine::general_purpose::STANDARD.encode(name.as_bytes());
    let wrapped = encoded
        .as_bytes()
        .chunks(APPLE_XAR_BASE64_LINE_LENGTH)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join("\r\n");
    assert!(wrapped.contains("\r\n"));

    let xml = format!(
        "<xar><toc><file id=\"1\"><type>file</type><name enctype=\"base64\">{wrapped}</name></file></toc></xar>"
    );

    let files = parse_toc_xml(xml.as_bytes()).unwrap();
    assert_eq!(files[0].name, name);
    assert_eq!(files[0].path, name);
}

#[test]
fn rejects_unsupported_name_enctype() {
    let xml = br#"<xar><toc><file id="1"><type>file</type><name enctype="quoted-printable">entry</name></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn rejects_invalid_base64_name() {
    let xml = br#"<xar><toc><file id="1"><type>file</type><name enctype="base64">not valid base64!!</name></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}

#[test]
fn rejects_base64_name_decoding_to_invalid_utf8() {
    // "//4=" is the base64 encoding of the two bytes 0xFF 0xFE, which is
    // not valid UTF-8.
    let xml = br#"<xar><toc><file id="1"><type>file</type><name enctype="base64">//4=</name></file></toc></xar>"#;
    assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
}
