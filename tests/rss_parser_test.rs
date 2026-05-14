use std::fs;

#[test]
fn test_parse_rss_first_update_fields() {
    let content = fs::read_to_string("tests/fixtures/rss.xml").expect("Failed to read rss.xml");
    let channel = rss::Channel::read_from(content.as_bytes()).expect("Failed to parse RSS XML");
    let updates = dst_update::parse_updates(&channel);

    assert_eq!(updates.len(), 5, "Should parse 5 items with valid pub_date");

    let first = &updates[0];
    assert_eq!(first.build_number, 530002);
    assert_eq!(first.release_id, 1902);
    assert_eq!(first.platform, dst_update::Platform::PC);
}

#[test]
fn test_parse_rss_platform_identification() {
    let content = fs::read_to_string("tests/fixtures/rss.xml").expect("Failed to read rss.xml");
    let channel = rss::Channel::read_from(content.as_bytes()).expect("Failed to parse RSS XML");
    let updates = dst_update::parse_updates(&channel);

    assert_eq!(updates[0].platform, dst_update::Platform::PC);
    assert_eq!(updates[1].platform, dst_update::Platform::PC);
    assert_eq!(updates[2].platform, dst_update::Platform::Xbox);
    assert_eq!(updates[3].platform, dst_update::Platform::PlayStation);
    assert_eq!(updates[4].platform, dst_update::Platform::Switch);
}

#[test]
fn test_parse_rss_filters_missing_pub_date() {
    let content = fs::read_to_string("tests/fixtures/rss.xml").expect("Failed to read rss.xml");
    let channel = rss::Channel::read_from(content.as_bytes()).expect("Failed to parse RSS XML");
    let updates = dst_update::parse_updates(&channel);

    let no_date: Vec<_> = updates
        .iter()
        .filter(|u| u.title == "Update without date")
        .collect();
    assert!(
        no_date.is_empty(),
        "Items without pub_date should be filtered out"
    );
}
