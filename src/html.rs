use chrono::NaiveDate;
use log::{debug, info, warn};
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use crate::http::{async_fetch, fetch};
use crate::model::{AppError, HtmlGameUpdate, HtmlUpdateDetail, UpdateStatus};
use crate::text::clean_html;

pub const BASE_URL: &str = "https://forums.kleientertainment.com/game-updates/dst";

pub fn page_url(page: u32) -> String {
    format!("{}/page/{}/", BASE_URL, page)
}

pub fn fetch_page(page: u32) -> Result<String, AppError> {
    let url = page_url(page);
    let html = fetch(&url)?;
    Ok(html)
}

pub async fn fetch_page_async(page: u32) -> Result<String, AppError> {
    let url = page_url(page);
    let html = async_fetch(&url).await?;
    Ok(html)
}

pub fn fetch_and_parse_page(page: u32) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let html = fetch_page(page)?;
    parse_update_html(&html)
}

pub fn parse_update_html(html: &str) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let document = Html::parse_document(html);
    let row_selector =
        Selector::parse("li.cCmsRecord_row").expect("CSS 选择器解析失败: li.cCmsRecord_row");
    let link_selector = Selector::parse("a.cRelease").expect("CSS 选择器解析失败: a.cRelease");
    let h3_selector = Selector::parse("h3.ipsType_sectionHead")
        .expect("CSS 选择器解析失败: h3.ipsType_sectionHead");
    let badge_selector =
        Selector::parse("span.ipsBadge").expect("CSS 选择器解析失败: span.ipsBadge");
    let meta_selector =
        Selector::parse("div.ipsDataItem_meta").expect("CSS 选择器解析失败: div.ipsDataItem_meta");
    let hotfix_selector =
        Selector::parse("span.cUpdate_hotfix").expect("CSS 选择器解析失败: span.cUpdate_hotfix");

    debug!("开始解析更新页面 HTML");

    let mut updates = Vec::new();

    for row in document.select(&row_selector) {
        let row_id: u32 = row
            .value()
            .attr("data-rowid")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if let Some(link_el) = row.select(&link_selector).next() {
            let link = link_el.value().attr("href").unwrap_or("").to_string();
            let release_id: u32 = link_el
                .value()
                .attr("data-releaseid")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if release_id == 0 {
                warn!("跳过 release_id=0 的行: row_id={}", row_id);
                continue;
            }

            let is_current = link_el.value().attr("data-currentrelease").is_some();
            let is_hotfix = link_el.select(&hotfix_selector).next().is_some();

            if let Some(h3) = link_el.select(&h3_selector).next() {
                let text = h3.text().next().map(|s| s.trim()).unwrap_or("");
                let build_number: u32 = match text.parse() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        warn!("跳过无法解析 build_number 的条目: text={:?}", text);
                        continue;
                    }
                };

                let status = link_el
                    .select(&badge_selector)
                    .next()
                    .map(|b| b.text().collect::<String>())
                    .map(|s| match s.trim() {
                        "Test" => UpdateStatus::Test,
                        "Release" => UpdateStatus::Release,
                        _ => UpdateStatus::Unknown,
                    })
                    .unwrap_or(UpdateStatus::Unknown);

                let release_date = match link_el
                    .select(&meta_selector)
                    .next()
                    .map(|m| m.text().collect::<String>())
                    .and_then(|text| parse_release_date(&text))
                {
                    Some(d) => d,
                    None => {
                        let raw = link_el
                            .select(&meta_selector)
                            .next()
                            .map(|m| m.text().collect::<String>());
                        warn!(
                            "跳过无法解析发布日期的条目: build={}, raw_meta={:?}",
                            build_number, raw
                        );
                        continue;
                    }
                };

                updates.push(HtmlGameUpdate {
                    build_number,
                    link,
                    release_date,
                    is_hotfix,
                    status,
                    is_current,
                    row_id,
                    release_id,
                });
            }
        }
    }

    debug!("解析到 {} 条更新", updates.len());

    Ok(updates)
}

pub fn latest_update(
    updates: &[HtmlGameUpdate],
    status_filter: Option<UpdateStatus>,
) -> Option<&HtmlGameUpdate> {
    updates
        .iter()
        .filter(|u| match status_filter {
            None => true,
            Some(UpdateStatus::Test) => {
                u.status == UpdateStatus::Test || u.status == UpdateStatus::Unknown
            }
            Some(UpdateStatus::Release) => {
                u.status == UpdateStatus::Release || u.status == UpdateStatus::Unknown
            }
            Some(UpdateStatus::Unknown) => true,
        })
        .max_by_key(|u| u.release_id)
}

/// 吸收一页解析结果：空页或整页均为已见条目时返回 false（应停止翻页）；
/// 否则将其中未见的条目追加到 `all` 并登记到 `seen`，返回 true。
fn absorb_page(
    seen: &mut HashSet<u32>,
    all: &mut Vec<HtmlGameUpdate>,
    updates: Vec<HtmlGameUpdate>,
) -> bool {
    if updates.is_empty() || updates.iter().all(|u| seen.contains(&u.release_id)) {
        return false;
    }
    all.extend(updates.into_iter().filter(|u| seen.insert(u.release_id)));
    true
}

pub fn fetch_all_updates(max_pages: Option<u32>) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let limit = max_pages.unwrap_or(100);
    let mut all_updates = Vec::new();
    let mut seen = HashSet::new();

    for page in 1..=limit {
        debug!("获取第 {} 页更新", page);
        let updates = fetch_and_parse_page(page)?;
        if !absorb_page(&mut seen, &mut all_updates, updates) {
            debug!("第 {} 页无新增条目，停止翻页", page);
            break;
        }
    }

    Ok(all_updates)
}

pub async fn fetch_all_updates_async(
    max_pages: Option<u32>,
) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let limit = max_pages.unwrap_or(100);
    let mut all_updates = Vec::new();
    let mut seen = HashSet::new();

    for page in 1..=limit {
        debug!("获取第 {} 页更新", page);
        let html = fetch_page_async(page).await?;
        let updates = parse_update_html(&html)?;
        if !absorb_page(&mut seen, &mut all_updates, updates) {
            debug!("第 {} 页无新增条目，停止翻页", page);
            break;
        }
    }

    Ok(all_updates)
}

pub fn parse_detail_html(html: &str) -> Result<(String, String), AppError> {
    let document = Html::parse_document(html);

    let title_selector =
        Selector::parse("h1.ipsType_pageTitle").expect("CSS selector parse failed");
    let title = document
        .select(&title_selector)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let content_selector =
        Selector::parse("section.ipsType_richText").expect("CSS selector parse failed");
    let content_text = document
        .select(&content_selector)
        .next()
        .map(|el| clean_html(&el.html()))
        .unwrap_or_default();

    let content_text = content_text
        .strip_prefix("Update Information:\n")
        .unwrap_or(&content_text)
        .to_string();

    if content_text.is_empty() {
        return Err(AppError::HtmlParse(
            "详情页内容为空或选择器不匹配".to_string(),
        ));
    }

    Ok((title, content_text))
}

pub fn fetch_and_parse_detail(update: &HtmlGameUpdate) -> Result<HtmlUpdateDetail, AppError> {
    let html = fetch(&update.link)?;
    let (title, content_text) = parse_detail_html(&html)?;

    Ok(HtmlUpdateDetail {
        build_number: update.build_number,
        link: update.link.clone(),
        release_date: update.release_date,
        is_hotfix: update.is_hotfix,
        status: update.status,
        is_current: update.is_current,
        row_id: update.row_id,
        release_id: update.release_id,
        title,
        content_text,
    })
}

pub async fn fetch_and_parse_detail_async(
    update: &HtmlGameUpdate,
) -> Result<HtmlUpdateDetail, AppError> {
    let html = async_fetch(&update.link).await?;
    let (title, content_text) = parse_detail_html(&html)?;

    Ok(HtmlUpdateDetail {
        build_number: update.build_number,
        link: update.link.clone(),
        release_date: update.release_date,
        is_hotfix: update.is_hotfix,
        status: update.status,
        is_current: update.is_current,
        row_id: update.row_id,
        release_id: update.release_id,
        title,
        content_text,
    })
}

pub fn compute_fetch_plan(
    updates: &[HtmlGameUpdate],
    existing: &[HtmlUpdateDetail],
) -> (HashMap<u32, HtmlUpdateDetail>, Vec<usize>) {
    let existing_map: HashMap<u32, HtmlUpdateDetail> = existing
        .iter()
        .filter(|d| !d.content_text.is_empty())
        .map(|d| (d.release_id, d.clone()))
        .collect();

    let to_fetch: Vec<usize> = updates
        .iter()
        .enumerate()
        .filter(|(_, u)| !existing_map.contains_key(&u.release_id))
        .map(|(i, _)| i)
        .collect();

    (existing_map, to_fetch)
}

pub async fn fetch_all_update_details(
    max_pages: Option<u32>,
    existing: Option<Vec<HtmlUpdateDetail>>,
    concurrency: usize,
    interval: Duration,
) -> Result<Vec<HtmlUpdateDetail>, AppError> {
    let updates = Arc::new(fetch_all_updates_async(max_pages).await?);

    let (existing_map, to_fetch) =
        compute_fetch_plan(&updates, existing.unwrap_or_default().as_slice());

    let skipped = updates.len() - to_fetch.len();

    if skipped > 0 {
        info!(
            "共 {} 条更新，其中 {} 条已有详情将跳过，需抓取 {} 条",
            updates.len(),
            skipped,
            to_fetch.len()
        );
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let total_to_fetch = to_fetch.len();
    let mut handles = Vec::with_capacity(total_to_fetch);

    for (seq, idx) in to_fetch.into_iter().enumerate() {
        let sem = semaphore.clone();
        let updates = updates.clone();
        let existing_map = existing_map.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if seq > 0 {
                tokio::time::sleep(interval).await;
            }

            let update = &updates[idx];
            debug!(
                "获取详情 {}/{}: build={}",
                seq + 1,
                total_to_fetch,
                update.build_number
            );

            match fetch_and_parse_detail_async(update).await {
                Ok(detail) => (update.release_id, Ok(detail)),
                Err(e) => {
                    warn!("获取详情失败 build={}: {}", update.build_number, e);
                    let detail = existing_map
                        .get(&update.release_id)
                        .cloned()
                        .unwrap_or(HtmlUpdateDetail {
                            build_number: update.build_number,
                            link: update.link.clone(),
                            release_date: update.release_date,
                            is_hotfix: update.is_hotfix,
                            status: update.status,
                            is_current: update.is_current,
                            row_id: update.row_id,
                            release_id: update.release_id,
                            title: String::new(),
                            content_text: String::new(),
                        });
                    (update.release_id, Err(detail))
                }
            }
        }));
    }

    let mut results: Vec<(u32, Result<HtmlUpdateDetail, HtmlUpdateDetail>)> = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    let fetched_map: HashMap<u32, HtmlUpdateDetail> = results
        .into_iter()
        .map(|(rid, r)| (rid, r.unwrap_or_else(|e| e)))
        .collect();

    let details: Vec<HtmlUpdateDetail> = updates
        .iter()
        .map(|u| {
            fetched_map
                .get(&u.release_id)
                .cloned()
                .unwrap_or_else(|| {
                    existing_map
                        .get(&u.release_id)
                        .cloned()
                        .unwrap_or(HtmlUpdateDetail {
                            build_number: u.build_number,
                            link: u.link.clone(),
                            release_date: u.release_date,
                            is_hotfix: u.is_hotfix,
                            status: u.status,
                            is_current: u.is_current,
                            row_id: u.row_id,
                            release_id: u.release_id,
                            title: String::new(),
                            content_text: String::new(),
                        })
                })
        })
        .collect();

    Ok(details)
}

pub fn fetch_latest_update(
    max_pages: u32,
    status_filter: Option<UpdateStatus>,
) -> Result<Option<HtmlGameUpdate>, AppError> {
    let mut best: Option<HtmlGameUpdate> = None;
    for page in 1..=max_pages {
        debug!("获取第 {} 页更新", page);
        let updates = fetch_and_parse_page(page)?;
        debug!("第 {} 页获取到 {} 条更新", page, updates.len());
        if let Some(latest) = latest_update(&updates, status_filter) {
            debug!(
                "第 {} 页找到最新更新: build={}, release_id={}",
                page, latest.build_number, latest.release_id
            );
            match &best {
                None => best = Some(latest.clone()),
                Some(current) if latest.release_id > current.release_id => {
                    best = Some(latest.clone());
                }
                Some(_) => {}
            }
        }
        if updates
            .last()
            .is_none_or(|u| best.as_ref().is_none_or(|b| u.release_id < b.release_id))
        {
            debug!("页面无更多更新或已找到最佳结果，停止翻页");
            break;
        }
    }
    Ok(best)
}

fn parse_release_date(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    let text = text.strip_prefix("Released ")?;
    let text = text.trim_end_matches("...");
    NaiveDate::parse_from_str(text, "%m/%d/%y").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_update(build_number: u32, release_id: u32, status: UpdateStatus) -> HtmlGameUpdate {
        HtmlGameUpdate {
            build_number,
            link: String::new(),
            release_date: chrono::Utc::now().date_naive(),
            is_hotfix: false,
            status,
            is_current: false,
            row_id: 0,
            release_id,
        }
    }

    #[test]
    fn test_latest_update_no_filter() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Test),
            make_update(200, 2, UpdateStatus::Release),
        ];
        let result = latest_update(&updates, None);
        assert_eq!(result.unwrap().release_id, 2);
    }

    #[test]
    fn test_latest_update_filter_test() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
            make_update(300, 3, UpdateStatus::Unknown),
        ];
        let result = latest_update(&updates, Some(UpdateStatus::Test));
        assert_eq!(result.unwrap().release_id, 3);
    }

    #[test]
    fn test_latest_update_filter_release() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Test),
            make_update(200, 2, UpdateStatus::Release),
            make_update(300, 3, UpdateStatus::Unknown),
        ];
        let result = latest_update(&updates, Some(UpdateStatus::Release));
        assert_eq!(result.unwrap().release_id, 3);
    }

    #[test]
    fn test_latest_update_filter_excludes_non_matching() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
        ];
        let result = latest_update(&updates, Some(UpdateStatus::Test));
        assert_eq!(result.unwrap().release_id, 2);
    }

    #[test]
    fn test_latest_update_empty() {
        let updates: Vec<HtmlGameUpdate> = vec![];
        assert!(latest_update(&updates, None).is_none());
    }

    #[test]
    fn test_latest_update_all_filtered_out() {
        let updates = vec![make_update(100, 1, UpdateStatus::Release)];
        assert!(latest_update(&updates, Some(UpdateStatus::Test)).is_none());
    }

    #[test]
    fn test_parse_release_date_valid() {
        let result = parse_release_date("Released 11/05/22");
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "2022-11-05");
    }

    #[test]
    fn test_parse_release_date_with_dots() {
        let result = parse_release_date("Released 11/05/22...");
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "2022-11-05");
    }

    #[test]
    fn test_parse_release_date_invalid() {
        assert!(parse_release_date("Not a date").is_none());
    }

    #[test]
    fn test_parse_release_date_missing_prefix() {
        assert!(parse_release_date("11/05/22").is_none());
    }

    #[test]
    fn test_page_url() {
        assert_eq!(
            page_url(3),
            "https://forums.kleientertainment.com/game-updates/dst/page/3/"
        );
    }

    #[test]
    fn test_parse_detail_html_normal() {
        let html = std::fs::read_to_string("tests/fixtures/detail.html")
            .expect("Failed to read detail.html");
        let (title, content) = parse_detail_html(&html).expect("Failed to parse detail HTML");

        assert_eq!(title, "529998 - Test Update");
        assert!(!content.starts_with("Update Information:\n"), "Should strip prefix");
        assert!(content.contains("Fixed a crash when saving"));
        assert!(content.contains("Added new feature XYZ"));
        assert!(content.contains("See the full notes for details."));
    }

    #[test]
    fn test_parse_detail_html_no_prefix() {
        let html = std::fs::read_to_string("tests/fixtures/detail_no_prefix.html")
            .expect("Failed to read detail_no_prefix.html");
        let (title, content) = parse_detail_html(&html).expect("Failed to parse detail HTML");

        assert_eq!(title, "530002 - Release Update");
        assert!(content.contains("Bug fixes for the release version."));
        assert!(content.contains("No prefix here."));
    }

    #[test]
    fn test_parse_detail_html_empty_content() {
        let html = std::fs::read_to_string("tests/fixtures/detail_empty.html")
            .expect("Failed to read detail_empty.html");
        let result = parse_detail_html(&html);
        assert!(result.is_err(), "Should return error for empty content");
    }

    #[test]
    fn test_parse_detail_html_strips_update_information_prefix() {
        let html = r#"<html><body>
            <h1 class="ipsType_pageTitle">Test Title</h1>
            <section class="ipsType_richText">
                <h2>Update Information:</h2>
                <p>Hello world</p>
            </section>
        </body></html>"#;
        let (_, content) = parse_detail_html(html).unwrap();
        assert!(content.starts_with("Hello world"), "Content should start after prefix, got: {:?}", content);
    }

    #[test]
    fn test_parse_detail_html_inline_h2_no_strip() {
        let html = r#"<html><body>
            <h1 class="ipsType_pageTitle">Test Title</h1>
            <section class="ipsType_richText"><h2>Update Information:</h2><p>Hello world</p></section>
        </body></html>"#;
        let (_, content) = parse_detail_html(html).unwrap();
        assert!(
            content.contains("Hello world"),
            "Content should contain body text, got: {:?}",
            content
        );
        assert!(
            content.contains("Update Information:"),
            "Inline h2 without newline prevents prefix stripping, got: {:?}",
            content
        );
    }

    fn make_detail(release_id: u32, content_text: &str) -> HtmlUpdateDetail {
        HtmlUpdateDetail {
            build_number: 500000 + release_id,
            link: format!("https://example.com/{}", release_id),
            release_date: chrono::Utc::now().date_naive(),
            is_hotfix: false,
            status: UpdateStatus::Release,
            is_current: false,
            row_id: release_id,
            release_id,
            title: format!("Update {}", release_id),
            content_text: content_text.to_string(),
        }
    }

    #[test]
    fn test_compute_fetch_plan_all_new() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
        ];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &[]);
        assert!(existing_map.is_empty());
        assert_eq!(to_fetch, vec![0, 1]);
    }

    #[test]
    fn test_compute_fetch_plan_skip_existing_with_content() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
            make_update(300, 3, UpdateStatus::Release),
        ];
        let existing = vec![
            make_detail(1, "some content"),
            make_detail(3, "other content"),
        ];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &existing);
        assert_eq!(existing_map.len(), 2);
        assert!(existing_map.contains_key(&1));
        assert!(existing_map.contains_key(&3));
        assert_eq!(to_fetch, vec![1]);
    }

    #[test]
    fn test_compute_fetch_plan_empty_content_not_skipped() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
        ];
        let existing = vec![make_detail(1, "")];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &existing);
        assert!(
            existing_map.is_empty(),
            "Empty content_text should not be in existing_map"
        );
        assert_eq!(to_fetch, vec![0, 1]);
    }

    #[test]
    fn test_compute_fetch_plan_partial_empty_content() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
            make_update(300, 3, UpdateStatus::Release),
        ];
        let existing = vec![
            make_detail(1, "valid content"),
            make_detail(2, ""),
        ];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &existing);
        assert_eq!(existing_map.len(), 1);
        assert!(existing_map.contains_key(&1));
        assert_eq!(to_fetch, vec![1, 2]);
    }

    #[test]
    fn test_compute_fetch_plan_no_updates() {
        let updates: Vec<HtmlGameUpdate> = vec![];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &[]);
        assert!(existing_map.is_empty());
        assert!(to_fetch.is_empty());
    }

    #[test]
    fn test_compute_fetch_plan_existing_not_in_updates() {
        let updates = vec![make_update(100, 1, UpdateStatus::Release)];
        let existing = vec![make_detail(99, "orphan content")];
        let (existing_map, to_fetch) = compute_fetch_plan(&updates, &existing);
        assert_eq!(existing_map.len(), 1);
        assert!(existing_map.contains_key(&99));
        assert_eq!(to_fetch, vec![0]);
    }

    #[test]
    fn test_absorb_page_stops_on_empty_page() {
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let continued = absorb_page(&mut seen, &mut all, vec![]);
        assert!(!continued);
        assert!(all.is_empty());
        assert!(seen.is_empty());
    }

    #[test]
    fn test_absorb_page_accepts_fresh_page() {
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let page = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Release),
        ];
        assert!(absorb_page(&mut seen, &mut all, page));
        assert_eq!(all.len(), 2);
        assert!(seen.contains(&1) && seen.contains(&2));
    }

    #[test]
    fn test_absorb_page_stops_when_all_entries_seen() {
        // CMS 在真实数据耗尽后重复返回旧内容：整页均已见 → 停止且不追加
        let mut seen = HashSet::from([1, 2]);
        let mut all = vec![make_update(100, 1, UpdateStatus::Release)];
        let len_before = all.len();
        let page = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Release),
        ];
        assert!(!absorb_page(&mut seen, &mut all, page));
        assert_eq!(all.len(), len_before);
    }

    #[test]
    fn test_absorb_page_dedups_within_page() {
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let page = vec![
            make_update(100, 7, UpdateStatus::Release),
            make_update(100, 7, UpdateStatus::Release),
            make_update(200, 8, UpdateStatus::Release),
        ];
        assert!(absorb_page(&mut seen, &mut all, page));
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].release_id, 7);
        assert_eq!(all[1].release_id, 8);
    }

    #[test]
    fn test_absorb_page_partial_new_continues() {
        // 页内新旧混合（翻页边界漂移）：继续翻页，但只吸收新条目
        let mut seen = HashSet::from([1]);
        let mut all = Vec::new();
        let page = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(300, 9, UpdateStatus::Release),
        ];
        assert!(absorb_page(&mut seen, &mut all, page));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].release_id, 9);
    }
}
