use axum::{
    http::{HeaderMap, HeaderValue},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::types::PaginationParams;

pub fn paginate_params(params: &PaginationParams) -> (u32, u32, i64, i64) {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let offset = (page - 1) as i64 * per_page as i64;
    let limit = per_page as i64;
    (page, per_page, limit, offset)
}

fn build_link_header(base_path: &str, page: u32, per_page: u32, total: i64) -> Option<String> {
    let total_pages = ((total as f64) / (per_page as f64)).ceil().max(1.0) as u32;

    if total_pages <= 1 && page == 1 {
        return None;
    }

    let mut links = Vec::new();

    if page < total_pages {
        links.push(format!(
            "<{}?page={}&per_page={}>; rel=\"next\"",
            base_path,
            page + 1,
            per_page
        ));
    }

    if page > 1 {
        links.push(format!(
            "<{}?page={}&per_page={}>; rel=\"prev\"",
            base_path,
            page - 1,
            per_page
        ));
    }

    links.push(format!(
        "<{}?page=1&per_page={}>; rel=\"first\"",
        base_path, per_page
    ));

    links.push(format!(
        "<{}?page={}&per_page={}>; rel=\"last\"",
        base_path, total_pages, per_page
    ));

    Some(links.join(", "))
}

pub fn paginated_response<T: Serialize>(
    items: Vec<T>,
    page: u32,
    per_page: u32,
    total: i64,
    base_path: &str,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();

    if let Some(link) = build_link_header(base_path, page, per_page, total) {
        if let Ok(val) = HeaderValue::from_str(&link) {
            headers.insert("link", val);
        }
    }

    if let Ok(val) = HeaderValue::from_str(&total.to_string()) {
        headers.insert("x-total-count", val);
    }

    (headers, Json(items))
}
