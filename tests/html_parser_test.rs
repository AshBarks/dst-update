use chrono::Datelike;
use std::fs;

#[test]
fn test_parse_html_first_update_fields() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");

    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    assert!(!updates.is_empty(), "Should parse at least one update");

    let first = &updates[0];
    assert_eq!(first.build_number, 529998);
    assert_eq!(first.row_id, 1899);
    assert_eq!(first.release_id, 1899);
    assert!(first.is_hotfix);
    assert_eq!(first.status, dst_update::UpdateStatus::Test);
    assert!(!first.is_current);
    assert_eq!(
        first.link,
        "https://forums.kleientertainment.com/game-updates/dst/529998-r1899/"
    );
    assert_eq!(first.release_date.to_string(), "2022-11-05");
}

#[test]
fn test_parse_html_variety_of_updates() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");

    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    assert!(updates.len() > 10, "Should parse multiple updates");

    let hotfix_count = updates.iter().filter(|u| u.is_hotfix).count();
    assert!(hotfix_count > 0, "Should have some hotfix updates");

    let release_count = updates
        .iter()
        .filter(|u| u.status == dst_update::UpdateStatus::Release)
        .count();
    assert!(release_count > 0, "Should have some release updates");

    let current_count = updates.iter().filter(|u| u.is_current).count();
    assert!(current_count > 0, "Should have some current releases");
}

#[test]
fn test_parse_html_current_release_is_release_status() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");

    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    let current_releases: Vec<_> = updates.iter().filter(|u| u.is_current).collect();
    assert!(!current_releases.is_empty(), "Should have current releases");

    for update in current_releases {
        assert_eq!(
            update.status,
            dst_update::UpdateStatus::Release,
            "Current releases should have Release status"
        );
    }
}

#[test]
fn test_parse_html_release_date_fields() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");

    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    let dates: Vec<_> = updates.iter().map(|u| u.release_date).collect();
    assert!(
        dates.iter().all(|d| d.year() >= 2022),
        "All dates should be in 2022 or later"
    );

    let first = &updates[0];
    assert_eq!(first.release_date.month(), 11);
    assert_eq!(first.release_date.day(), 5);
}

#[test]
fn test_parse_empty_html() {
    let html = "<html><body></body></html>";
    let updates = dst_update::parse_update_html(html).expect("Failed to parse HTML");
    assert!(updates.is_empty(), "Should return empty vec for empty HTML");
}

#[test]
fn test_parse_html_row_id_descending_order() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");

    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    assert!(updates.len() >= 3, "Need at least 3 updates to test order");

    assert!(
        updates[0].row_id >= updates[1].row_id,
        "Updates should be in descending order by row_id"
    );
}

#[test]
fn test_html_latest_update_no_filter() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");
    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    let latest = dst_update::latest_html_update(&updates, None);
    assert!(latest.is_some());

    let latest = latest.unwrap();
    assert!(latest.release_id > 0);
    assert_eq!(
        latest.release_id,
        updates.iter().map(|u| u.release_id).max().unwrap()
    );
}

#[test]
fn test_html_latest_update_filter_release() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");
    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    let latest = dst_update::latest_html_update(&updates, Some(dst_update::UpdateStatus::Release));
    assert!(latest.is_some());

    let latest = latest.unwrap();
    assert!(
        latest.status == dst_update::UpdateStatus::Release
            || latest.status == dst_update::UpdateStatus::Unknown
    );
}

#[test]
fn test_html_latest_update_filter_test() {
    let html =
        fs::read_to_string("tests/fixtures/update.html").expect("Failed to read update.html");
    let updates = dst_update::parse_update_html(&html).expect("Failed to parse HTML");

    let latest = dst_update::latest_html_update(&updates, Some(dst_update::UpdateStatus::Test));
    assert!(latest.is_some());

    let latest = latest.unwrap();
    assert!(
        latest.status == dst_update::UpdateStatus::Test
            || latest.status == dst_update::UpdateStatus::Unknown
    );
}
