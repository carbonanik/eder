use super::*;

impl Eder {
    pub(crate) fn load(&mut self) {
        let task = self.app.begin_listing();
        self.runtime
            .thumb_gen
            .store(self.app.listing, Ordering::Release);
        self.ui.pending.clear();
        self.ui.search_listing = 0;
        self.app.loading = self.send(task);
    }
    pub(crate) fn touch(&mut self, p: &PathBuf) {
        if let Some(i) = self.ui.lru.iter().position(|x| x == p) {
            self.ui.lru.remove(i);
        }
        self.ui.lru.push_back(p.clone());
    }
    pub(crate) fn change_view(&mut self, view: View) {
        if self.app.change_view(view) {
            self.ui.search.clear();
            if view != View::Settings {
                self.load();
            }
        }
    }

    pub(crate) fn browse_folder(&mut self, folder: PathBuf) {
        self.app.browse_folder(folder);
        self.ui.search.clear();
        self.load();
    }

    pub(crate) fn pick_pdfs(&mut self, ctx: &egui::Context) {
        if self.app.state.root.is_none() {
            self.change_view(View::Settings);
            self.app.message = Some((
                "Choose your library folder before adding a PDF.".into(),
                false,
            ));
            return;
        }
        if self.ui.file_dialog.is_some() {
            return;
        }
        self.ui.file_dialog = Some(crate::infrastructure::platform::pick_files({
            let ctx = ctx.clone();
            Arc::new(move || ctx.request_repaint())
        }));
    }

    pub(crate) fn pick_library_folder(&mut self, ctx: &egui::Context) {
        self.ui.dialog = Some(crate::infrastructure::platform::pick_folder({
            let ctx = ctx.clone();
            Arc::new(move || ctx.request_repaint())
        }));
    }
    pub(crate) fn refresh(&mut self) {
        self.ui.textures.clear();
        self.ui.lru.clear();
        self.load();
    }
}
