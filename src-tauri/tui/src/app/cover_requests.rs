use crate::action::{CoverRenderRequest, CoverSurface};

use super::AppState;

impl AppState {
    pub(super) fn cover_result_is_current(&self, request: &CoverRenderRequest) -> bool {
        let generation = match request.surface {
            CoverSurface::Playing | CoverSurface::Bar => self.generation,
            CoverSurface::Selection => self.selected_cover.generation,
        };
        request.generation == generation
            && request.style_revision == self.style_revision
            && self.cover_requests.is_current(request)
    }
}

#[derive(Default)]
pub(super) struct CoverRequests {
    next_id: u64,
    playing: Option<CoverRenderRequest>,
    bar: Option<CoverRenderRequest>,
    selection: Option<CoverRenderRequest>,
}

impl CoverRequests {
    pub(super) fn invalidate(&mut self) {
        self.playing = None;
        self.bar = None;
        self.selection = None;
    }

    pub(super) fn record(&mut self, request: &mut CoverRenderRequest) {
        self.next_id = self.next_id.wrapping_add(1);
        request.request_id = self.next_id;
        *self.slot_mut(request.surface) = Some(request.clone());
    }

    pub(super) fn is_current(&self, request: &CoverRenderRequest) -> bool {
        self.get(request.surface) == Some(request)
    }

    pub(super) fn forget(&mut self, surface: CoverSurface) {
        *self.slot_mut(surface) = None;
    }

    pub(super) fn needs_original(&self, request: &CoverRenderRequest) -> bool {
        self.get(request.surface).is_none_or(|current| {
            current.generation != request.generation
                || current.source_edge != request.source_edge
                || current.display_edge < request.display_edge
        })
    }

    fn get(&self, surface: CoverSurface) -> Option<&CoverRenderRequest> {
        match surface {
            CoverSurface::Playing => self.playing.as_ref(),
            CoverSurface::Bar => self.bar.as_ref(),
            CoverSurface::Selection => self.selection.as_ref(),
        }
    }

    fn slot_mut(&mut self, surface: CoverSurface) -> &mut Option<CoverRenderRequest> {
        match surface {
            CoverSurface::Playing => &mut self.playing,
            CoverSurface::Bar => &mut self.bar,
            CoverSurface::Selection => &mut self.selection,
        }
    }
}
