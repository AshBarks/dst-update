mod html;
mod http;
mod model;
mod rss;
mod text;

pub use html::{
    ExportOptions, dedup_details, fetch_all_update_details, fetch_all_updates,
    fetch_and_parse_detail, fetch_and_parse_detail_async, fetch_latest_update,
    latest_update as latest_html_update, parse_update_html,
};
pub use model::{
    AppError, HtmlGameUpdate, HtmlUpdateDetail, Platform, RssUpdateInfo, UpdateExport, UpdateStatus,
};
pub use rss::{RSS_URL, fetch_rss, latest_update as latest_rss_update, parse_updates};
pub use text::clean_html;
