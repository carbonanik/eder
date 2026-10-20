use crate::domain::{document::PdfLink, pixels::Pixels};
use std::path::PathBuf;

pub struct PageJob {
    pub key: crate::domain::document::RenderKey,
    pub id: u64,
}
pub struct PageResult<P = Pixels> {
    pub key: crate::domain::document::RenderKey,
    pub id: u64,
    pub result: Result<P, crate::domain::error::AppError>,
}
pub struct LinkJob {
    pub path: PathBuf,
    pub page: u32,
    pub total: u32,
    pub id: u64,
}
pub struct LinkResult {
    pub id: u64,
    pub page: u32,
    pub links: Vec<PdfLink>,
}
pub struct ThumbJob {
    pub path: PathBuf,
    pub id: u64,
}
pub struct ThumbResult<P = Pixels> {
    pub path: PathBuf,
    pub id: u64,
    pub image: Option<P>,
}
