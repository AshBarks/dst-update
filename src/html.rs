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

/// 吸收一页解析结果：空页或整页均为已见/已知条目时返回 false（应停止翻页）；
/// 否则将其中未见的条目追加到 `all` 并登记到 `seen`，返回 true。
/// `known` 是导出文件中有效条目的增量水位线：命中会触发停止，但不影响吸收。
fn absorb_page(
    seen: &mut HashSet<u32>,
    known: &HashSet<u32>,
    all: &mut Vec<HtmlGameUpdate>,
    updates: Vec<HtmlGameUpdate>,
) -> bool {
    if updates.is_empty()
        || updates
            .iter()
            .all(|u| seen.contains(&u.release_id) || known.contains(&u.release_id))
    {
        return false;
    }
    all.extend(updates.into_iter().filter(|u| seen.insert(u.release_id)));
    true
}

pub fn fetch_all_updates(
    max_pages: Option<u32>,
    known: &HashSet<u32>,
) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let limit = max_pages.unwrap_or(100);
    let mut all_updates = Vec::new();
    let mut seen = HashSet::new();

    for page in 1..=limit {
        debug!("获取第 {} 页更新", page);
        let updates = fetch_and_parse_page(page)?;
        if !absorb_page(&mut seen, known, &mut all_updates, updates) {
            debug!("第 {} 页无新增条目，停止翻页", page);
            break;
        }
    }

    Ok(all_updates)
}

pub async fn fetch_all_updates_async(
    max_pages: Option<u32>,
    known: &HashSet<u32>,
) -> Result<Vec<HtmlGameUpdate>, AppError> {
    let limit = max_pages.unwrap_or(100);
    let mut all_updates = Vec::new();
    let mut seen = HashSet::new();

    for page in 1..=limit {
        debug!("获取第 {} 页更新", page);
        let html = fetch_page_async(page).await?;
        let updates = parse_update_html(&html)?;
        if !absorb_page(&mut seen, known, &mut all_updates, updates) {
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

fn valid_existing(existing: &[HtmlUpdateDetail]) -> HashMap<u32, HtmlUpdateDetail> {
    existing
        .iter()
        .filter(|d| !d.content_text.is_empty())
        .map(|d| (d.release_id, d.clone()))
        .collect()
}

pub fn dedup_details(details: Vec<HtmlUpdateDetail>) -> Vec<HtmlUpdateDetail> {
    let mut seen = HashSet::new();
    details
        .into_iter()
        .filter(|d| seen.insert(d.release_id))
        .collect()
}

fn placeholder_detail(update: &HtmlGameUpdate) -> HtmlUpdateDetail {
    HtmlUpdateDetail {
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
    }
}

fn pending_refetch(
    existing: &[HtmlUpdateDetail],
    crawled_ids: &HashSet<u32>,
) -> Vec<HtmlGameUpdate> {
    existing
        .iter()
        .filter(|d| {
            d.content_text.is_empty() && !crawled_ids.contains(&d.release_id) && !d.link.is_empty()
        })
        .map(|d| HtmlGameUpdate {
            build_number: d.build_number,
            link: d.link.clone(),
            release_date: d.release_date,
            is_hotfix: d.is_hotfix,
            status: d.status,
            is_current: d.is_current,
            row_id: d.row_id,
            release_id: d.release_id,
        })
        .collect()
}

fn merge_details(
    crawled: &[HtmlGameUpdate],
    fetched: &HashMap<u32, HtmlUpdateDetail>,
    existing_map: &HashMap<u32, HtmlUpdateDetail>,
    existing: &[HtmlUpdateDetail],
) -> Vec<HtmlUpdateDetail> {
    let mut emitted = HashSet::new();
    let mut merged: Vec<HtmlUpdateDetail> = crawled
        .iter()
        .map(|u| {
            emitted.insert(u.release_id);
            fetched
                .get(&u.release_id)
                .cloned()
                .or_else(|| existing_map.get(&u.release_id).cloned())
                .unwrap_or_else(|| placeholder_detail(u))
        })
        .collect();

    for detail in existing {
        if emitted.insert(detail.release_id) {
            merged.push(
                fetched
                    .get(&detail.release_id)
                    .cloned()
                    .unwrap_or_else(|| detail.clone()),
            );
        }
    }

    merged
}

pub fn compute_fetch_plan(
    updates: &[HtmlGameUpdate],
    existing_map: &HashMap<u32, HtmlUpdateDetail>,
) -> Vec<usize> {
    updates
        .iter()
        .enumerate()
        .filter(|(_, u)| !existing_map.contains_key(&u.release_id))
        .map(|(i, _)| i)
        .collect()
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub max_pages: Option<u32>,
    pub concurrency: usize,
    pub interval: Duration,
    pub full: bool,
}

pub async fn fetch_all_update_details(
    existing: Option<Vec<HtmlUpdateDetail>>,
    opts: &ExportOptions,
) -> Result<Vec<HtmlUpdateDetail>, AppError> {
    let existing = dedup_details(existing.unwrap_or_default());
    let existing_map = valid_existing(&existing);
    let crawl_known = if opts.full {
        HashSet::new()
    } else {
        existing_map.keys().copied().collect()
    };

    let updates = fetch_all_updates_async(opts.max_pages, &crawl_known).await?;
    let crawled_ids: HashSet<u32> = updates.iter().map(|u| u.release_id).collect();

    let to_fetch_idx = compute_fetch_plan(&updates, &existing_map);
    let skipped = updates.len() - to_fetch_idx.len();
    let mut tasks: Vec<HtmlGameUpdate> = to_fetch_idx.iter().map(|&i| updates[i].clone()).collect();

    let prescan = pending_refetch(&existing, &crawled_ids);
    if !prescan.is_empty() {
        info!("预扫发现 {} 条失败占位条目，直接补抓详情", prescan.len());
    }
    tasks.extend(prescan);

    if skipped > 0 || !tasks.is_empty() {
        info!(
            "共 {} 条更新，其中 {} 条已有详情将跳过，需抓取 {} 条",
            updates.len(),
            skipped,
            tasks.len()
        );
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(opts.concurrency));
    let interval = opts.interval;
    let total_to_fetch = tasks.len();
    let mut handles = Vec::with_capacity(total_to_fetch);

    for (seq, update) in tasks.into_iter().enumerate() {
        let sem = semaphore.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if seq > 0 {
                tokio::time::sleep(interval).await;
            }

            debug!(
                "获取详情 {}/{}: build={}",
                seq + 1,
                total_to_fetch,
                update.build_number
            );

            match fetch_and_parse_detail_async(&update).await {
                Ok(detail) => (update.release_id, Ok(detail)),
                Err(e) => {
                    warn!("获取详情失败 build={}: {}", update.build_number, e);
                    (update.release_id, Err(placeholder_detail(&update)))
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

    Ok(merge_details(
        &updates,
        &fetched_map,
        &existing_map,
        &existing,
    ))
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
        assert!(
            !content.starts_with("Update Information:\n"),
            "Should strip prefix"
        );
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
        assert!(
            content.starts_with("Hello world"),
            "Content should start after prefix, got: {:?}",
            content
        );
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
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&[]));
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
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&existing));
        assert_eq!(to_fetch, vec![1]);
    }

    #[test]
    fn test_compute_fetch_plan_empty_content_not_skipped() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
        ];
        let existing = vec![make_detail(1, "")];
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&existing));
        assert_eq!(to_fetch, vec![0, 1], "空 content_text 不算有效已有条目");
    }

    #[test]
    fn test_compute_fetch_plan_partial_empty_content() {
        let updates = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Test),
            make_update(300, 3, UpdateStatus::Release),
        ];
        let existing = vec![make_detail(1, "valid content"), make_detail(2, "")];
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&existing));
        assert_eq!(to_fetch, vec![1, 2]);
    }

    #[test]
    fn test_compute_fetch_plan_no_updates() {
        let updates: Vec<HtmlGameUpdate> = vec![];
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&[]));
        assert!(to_fetch.is_empty());
    }

    #[test]
    fn test_compute_fetch_plan_existing_not_in_updates() {
        let updates = vec![make_update(100, 1, UpdateStatus::Release)];
        let existing = vec![make_detail(99, "orphan content")];
        let to_fetch = compute_fetch_plan(&updates, &valid_existing(&existing));
        assert_eq!(to_fetch, vec![0]);
    }

    #[test]
    fn test_absorb_page_stops_on_empty_page() {
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let continued = absorb_page(&mut seen, &HashSet::new(), &mut all, vec![]);
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
        assert!(absorb_page(&mut seen, &HashSet::new(), &mut all, page));
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
        assert!(!absorb_page(&mut seen, &HashSet::new(), &mut all, page));
        assert_eq!(all.len(), len_before);
    }

    #[test]
    fn test_absorb_page_stops_when_all_entries_known() {
        // 增量水位线：整页均为导出文件中已有的有效条目 → 停止翻页
        let known = HashSet::from([1, 2]);
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let page = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Release),
        ];
        assert!(!absorb_page(&mut seen, &known, &mut all, page));
        assert!(all.is_empty());
        assert!(seen.is_empty());
    }

    #[test]
    fn test_absorb_page_mixed_known_and_new_continues() {
        // 页内已知+新条目混合（新更新恰好跨页）：继续翻页，已知条目仍吸收进结果
        let known = HashSet::from([1]);
        let mut seen = HashSet::new();
        let mut all = Vec::new();
        let page = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(300, 9, UpdateStatus::Release),
        ];
        assert!(absorb_page(&mut seen, &known, &mut all, page));
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|u| u.release_id == 1));
        assert!(all.iter().any(|u| u.release_id == 9));
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
        assert!(absorb_page(&mut seen, &HashSet::new(), &mut all, page));
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
        assert!(absorb_page(&mut seen, &HashSet::new(), &mut all, page));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].release_id, 9);
    }

    #[test]
    fn test_valid_existing_filters_empty_content() {
        let existing = vec![make_detail(1, "valid"), make_detail(2, "")];
        let map = valid_existing(&existing);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&1));
    }

    #[test]
    fn test_dedup_details_keeps_first_occurrence() {
        let details = vec![
            make_detail(1, "first"),
            make_detail(2, "mid"),
            make_detail(1, "duplicate"),
        ];
        let deduped = dedup_details(details);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].content_text, "first");
        assert_eq!(deduped[1].release_id, 2);
    }

    #[test]
    fn test_pending_refetch_includes_placeholders_not_in_crawl() {
        let existing = vec![make_detail(1, "valid"), make_detail(5, "")];
        let crawled_ids = HashSet::from([1, 2, 3]);
        let pending = pending_refetch(&existing, &crawled_ids);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].release_id, 5);
        assert_eq!(pending[0].build_number, 500005);
        assert_eq!(pending[0].link, "https://example.com/5");
    }

    #[test]
    fn test_pending_refetch_excludes_crawled_valid_and_linkless() {
        let mut linkless = make_detail(7, "");
        linkless.link = String::new();
        let existing = vec![make_detail(1, "valid"), make_detail(5, ""), linkless];
        let crawled_ids = HashSet::from([5]);
        let pending = pending_refetch(&existing, &crawled_ids);
        assert!(
            pending.is_empty(),
            "有效条目/已在爬取区/无链接的占位均不应入列"
        );
    }

    #[test]
    fn test_merge_details_head_prefers_fetched_then_existing_map() {
        let crawled = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Release),
        ];
        let mut fetched = HashMap::new();
        fetched.insert(1, make_detail(1, "fresh"));
        let existing = vec![make_detail(2, "cached")];
        let existing_map = valid_existing(&existing);

        let merged = merge_details(&crawled, &fetched, &existing_map, &existing);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].content_text, "fresh");
        assert_eq!(merged[1].content_text, "cached");
    }

    #[test]
    fn test_merge_details_head_builds_placeholder_for_unknown() {
        let crawled = vec![make_update(100, 9, UpdateStatus::Release)];
        let merged = merge_details(&crawled, &HashMap::new(), &HashMap::new(), &[]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].release_id, 9);
        assert_eq!(merged[0].title, "");
        assert_eq!(merged[0].content_text, "");
    }

    #[test]
    fn test_merge_details_tail_appends_uncrawled_in_order() {
        let crawled = vec![make_update(200, 2, UpdateStatus::Release)];
        let existing = vec![make_detail(1, "old-1"), make_detail(3, "old-3")];
        let existing_map = valid_existing(&existing);

        let merged = merge_details(&crawled, &HashMap::new(), &existing_map, &existing);
        let ids: Vec<u32> = merged.iter().map(|d| d.release_id).collect();
        assert_eq!(ids, vec![2, 1, 3], "头部为爬取区，尾部按文件原序追加");
    }

    #[test]
    fn test_merge_details_tail_substitutes_refetched_placeholder() {
        let crawled = vec![];
        let existing = vec![make_detail(5, "")];
        let mut fetched = HashMap::new();
        fetched.insert(5, make_detail(5, "refetched"));

        let merged = merge_details(&crawled, &fetched, &HashMap::new(), &existing);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].content_text, "refetched");
    }

    #[test]
    fn test_merge_details_tail_keeps_placeholder_when_refetch_failed() {
        let crawled = vec![];
        let existing = vec![make_detail(5, "")];
        let mut fetched = HashMap::new();
        fetched.insert(5, make_detail(5, ""));

        let merged = merge_details(&crawled, &fetched, &HashMap::new(), &existing);
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].content_text, "",
            "补抓失败保留空占位，等待下次预扫"
        );
    }

    #[test]
    fn test_merge_details_emits_overlap_once() {
        let crawled = vec![
            make_update(100, 1, UpdateStatus::Release),
            make_update(200, 2, UpdateStatus::Release),
        ];
        let existing = vec![make_detail(2, "cached-2"), make_detail(3, "old-3")];
        let existing_map = valid_existing(&existing);

        let merged = merge_details(&crawled, &HashMap::new(), &existing_map, &existing);
        let ids: Vec<u32> = merged.iter().map(|d| d.release_id).collect();
        assert_eq!(ids, vec![1, 2, 3], "重叠条目只在头部出现一次");
    }
}
