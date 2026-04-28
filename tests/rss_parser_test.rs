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

#[test]
fn test_rss_latest_update_is_highest_pc_release_id() {
    let content = fs::read_to_string("tests/fixtures/rss.xml").expect("Failed to read rss.xml");
    let channel = rss::Channel::read_from(content.as_bytes()).expect("Failed to parse RSS XML");
    let updates = dst_update::parse_updates(&channel);

    let latest = dst_update::latest_rss_update(&updates, None);
    assert!(latest.is_some());

    let latest = latest.unwrap();
    assert_eq!(latest.release_id, 1902);
    assert_eq!(latest.platform, dst_update::Platform::PC);
}

#[test]
fn test_rss_latest_update_ignores_non_pc() {
    let content = fs::read_to_string("tests/fixtures/rss.xml").expect("Failed to read rss.xml");
    let channel = rss::Channel::read_from(content.as_bytes()).expect("Failed to parse RSS XML");
    let updates = dst_update::parse_updates(&channel);

    let pc_updates: Vec<_> = updates
        .iter()
        .filter(|u| u.platform == dst_update::Platform::PC)
        .collect();
    let non_pc: Vec<_> = updates
        .iter()
        .filter(|u| u.platform != dst_update::Platform::PC)
        .collect();

    assert_eq!(pc_updates.len(), 2);
    assert_eq!(non_pc.len(), 3);

    let latest = dst_update::latest_rss_update(&updates, None).unwrap();
    assert!(
        latest.release_id > 1898,
        "Should pick the highest PC release_id, not the highest overall"
    );
}
