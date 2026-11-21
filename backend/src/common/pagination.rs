use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

pub const DEFAULT_PAGE: u64 = 1;
pub const DEFAULT_PAGE_SIZE: u64 = 20;
pub const MAX_PAGE_SIZE: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl SortOrder {
    pub fn as_mongo_dir(self) -> i32 {
        match self {
            Self::Asc => 1,
            Self::Desc => -1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PaginationParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}

impl PaginationParams {
    pub fn normalized_page(&self) -> u64 {
        self.page.unwrap_or(DEFAULT_PAGE).max(1)
    }

    pub fn normalized_page_size(&self) -> u64 {
        self.page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    pub fn skip(&self) -> u64 {
        (self.normalized_page().saturating_sub(1)) * self.normalized_page_size()
    }

    pub fn sort_order_or_default(&self) -> SortOrder {
        self.sort_order.unwrap_or(SortOrder::Desc)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, total: u64, page: u64, page_size: u64) -> Self {
        let total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(page_size.max(1))
        };
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}
