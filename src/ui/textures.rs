use super::*;

pub(crate) struct CachedPage {
    pub key: crate::domain::document::RenderKey,
    pub texture: TextureHandle,
}

impl super::state::UiState {
    pub fn touch_page(&mut self, page: u32, dark: bool) {
        if let Some(index) = self
            .pages
            .iter()
            .position(|item| item.key.page == page && item.key.dark == dark)
        {
            let item = self.pages.remove(index).unwrap();
            self.pages.push_back(item);
        }
    }
}

impl super::state::UiState {
    pub fn enforce_page_budget(&mut self) {
        self.enforce_budget(crate::policy::PAGE_CACHE_BUDGET);
    }
    fn enforce_budget(&mut self, budget: usize) {
        let mut bytes: usize = self
            .pages
            .iter()
            .map(|p| p.texture.size()[0] * p.texture.size()[1] * 4)
            .sum();
        while bytes > budget {
            let Some(page) = self.pages.pop_front() else {
                break;
            };
            bytes -= page.texture.size()[0] * page.texture.size()[1] * 4;
            if self
                .current_page
                .as_ref()
                .is_some_and(|texture| texture.id() == page.texture.id())
            {
                self.current_page = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_releases_the_active_alias_and_updates_lru_on_use() {
        let ctx = egui::Context::default();
        let mut state = super::super::state::UiState::new(load_logo(&ctx));
        for page in 1..=2 {
            let texture = ctx.load_texture(
                format!("page-{page}"),
                egui::ColorImage::new([2, 2], Color32::WHITE),
                egui::TextureOptions::LINEAR,
            );
            if page == 1 {
                state.current_page = Some(texture.clone());
            }
            state.pages.push_back(CachedPage {
                texture,
                key: crate::domain::document::RenderKey {
                    document: crate::domain::document::DocumentId::from_canonical(
                        "book.pdf".into(),
                    ),
                    revision: crate::domain::document::FileRevision {
                        bytes: 1,
                        modified_nanos: 1,
                    },
                    page,
                    edge: 2,
                    dark: false,
                    backend_version: 1,
                },
            });
        }
        state.touch_page(1, false);
        assert_eq!(state.pages.back().unwrap().key.page, 1);
        state.touch_page(2, false);
        state.enforce_budget(16);
        assert_eq!(state.pages.len(), 1);
        assert_eq!(state.pages[0].key.page, 2);
        assert!(state.current_page.is_none());
    }
}
