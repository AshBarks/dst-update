use chrono::NaiveDate;
use log::{debug, warn};
use scraper::{Html, Selector};

use crate::http::fetch;
use crate::model::{AppError, HtmlGameUpdate, UpdateStatus};

pub const BASE_URL: &str = "https://forums.kleientertainment.com/game-updates/dst";

pub fn page_url(page: u32) -> String {
    format!("{}/page/{}/", BASE_URL, page)
}

pub fn fetch_page(page: u32) -> Result<String, AppError> {
    let url = page_url(page);
    let html = fetch(&url)?;
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
}
