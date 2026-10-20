use crate::domain::{document::PdfLink, reading_position::ReadingPosition};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    time::Instant,
};

pub struct Reader {
    pub path: PathBuf,
    pub revision: crate::domain::document::FileRevision,
    pub geometry: Vec<crate::domain::document::PageGeometry>,
    pub page: u32,
    pub total: u32,
    pub loading: bool,
    pub closing: bool,
    pub zoom: f32,
    pub links: HashMap<u32, Vec<PdfLink>>,
    pub link_pending: HashSet<u32>,
    pub target_edge: u32,
    pub last_visible_page: Option<u32>,
    pub position_changed_at: Option<Instant>,
    pub pending_page: Option<u32>,
    pub queued_pages: VecDeque<u32>,
    pub failed: HashSet<u32>,
    pub scroll_to: Option<ReadingPosition>,
    pub scroll_fraction: f32,
    pub anchor_version: u32,
    pub horizontal: f32,
    pub layout: Option<layout::PageLayout>,
}
impl Reader {
    pub fn render_key(&self, page: u32, dark: bool) -> crate::domain::document::RenderKey {
        crate::domain::document::RenderKey {
            document: crate::domain::document::DocumentId::from_canonical(self.path.clone()),
            revision: self.revision,
            page,
            edge: self.target_edge,
            dark,
            backend_version: 1,
        }
    }
    pub fn reading_position(&self, continuous: bool) -> ReadingPosition {
        if self.anchor_version == 0 {
            ReadingPosition::new(
                self.page,
                if continuous {
                    self.scroll_fraction
                } else {
                    0.0
                },
            )
        } else {
            ReadingPosition::content(self.page, self.scroll_fraction, self.zoom, self.horizontal)
        }
    }
}

pub mod layout;
pub mod scheduler;

impl Reader {
    /// Returns whether an in-flight raster has become irrelevant to the viewport.
    pub fn plan_pages(
        &mut self,
        visible: &[u32],
        dark: bool,
        cached: &[crate::domain::document::RenderKey],
    ) -> bool {
        let Some(first) = visible.iter().copied().min() else {
            return false;
        };
        let last = visible.iter().copied().max().unwrap();
        let cancel = self.pending_page.is_some_and(|pending| {
            pending.saturating_add(4) < first || pending > last.saturating_add(4)
        });
        if cancel {
            self.pending_page = None;
        }
        let forward = self
            .last_visible_page
            .is_none_or(|previous| first >= previous);
        self.last_visible_page = Some(first);
        self.queued_pages.clear();
        for page in scheduler::desired_pages(visible, self.total, forward) {
            let available = cached.iter().any(|key| {
                key.page == page
                    && key.dark == dark
                    && key.edge >= self.target_edge
                    && key.document.path() == self.path
                    && key.revision == self.revision
            });
            if self.pending_page != Some(page) && !self.failed.contains(&page) && !available {
                self.queued_pages.push_back(page);
            }
        }
        cancel
    }
    pub fn next_job(&mut self, generation: u64, dark: bool) -> Option<crate::app::jobs::PageJob> {
        if self.pending_page.is_some() || self.closing {
            return None;
        }
        let page = self.queued_pages.pop_front()?;
        self.pending_page = Some(page);
        Some(crate::app::jobs::PageJob {
            key: self.render_key(page, dark),
            id: generation,
        })
    }
}
