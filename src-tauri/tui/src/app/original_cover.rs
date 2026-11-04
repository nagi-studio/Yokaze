use image::{DynamicImage, Rgba};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_image::picker::Picker;
use ratatui_image::thread::{ResizeRequest, ResizeResponse, ThreadProtocol};
use ratatui_image::{FontSize, StatefulImage};
use tokio::sync::mpsc;

struct CoverSlot {
    protocol: ThreadProtocol,
    generation: Option<u64>,
}

impl CoverSlot {
    fn new(requests: mpsc::UnboundedSender<ResizeRequest>) -> Self {
        Self {
            protocol: ThreadProtocol::new(requests, None),
            generation: None,
        }
    }

    fn clear(&mut self) {
        self.protocol.empty_protocol();
        self.generation = None;
    }
}

pub(super) struct OriginalCover {
    pub(super) picker: Picker,
    background: Option<Rgba<u8>>,
    // Slots never move protocols between workers: response ids are slot-local.
    slots: Vec<CoverSlot>,
    front: usize,
    pending: Option<usize>,
}

impl OriginalCover {
    pub(super) fn new(picker: Picker, requests: mpsc::UnboundedSender<ResizeRequest>) -> Self {
        Self {
            picker,
            background: None,
            slots: vec![CoverSlot::new(requests)],
            front: 0,
            pending: None,
        }
    }

    pub(super) fn buffered(
        picker: Picker,
        requests: mpsc::UnboundedSender<ResizeRequest>,
        pending_requests: mpsc::UnboundedSender<ResizeRequest>,
    ) -> Self {
        Self {
            picker,
            background: None,
            slots: vec![CoverSlot::new(requests), CoverSlot::new(pending_requests)],
            front: 0,
            pending: None,
        }
    }

    pub(super) fn generation(&self) -> Option<u64> {
        self.slots[self.front].generation
    }

    pub(super) fn protocol(&self) -> &ThreadProtocol {
        &self.slots[self.front].protocol
    }

    #[cfg(test)]
    pub(super) fn protocol_mut(&mut self) -> &mut ThreadProtocol {
        &mut self.slots[self.front].protocol
    }

    pub(super) fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.clear();
        }
        self.pending = None;
        self.front = 0;
    }

    pub(super) fn refresh_font_size(&mut self, font_size: FontSize) {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(font_size);
        picker.set_protocol_type(self.picker.protocol_type());
        picker.set_background_color(self.background);
        self.picker = picker;
        self.clear();
    }

    pub(super) fn replace(&mut self, generation: u64, image: DynamicImage) {
        let slot = if self.generation().is_some() && self.slots.len() == 2 {
            let back = 1 - self.front;
            self.pending = Some(back);
            back
        } else {
            self.front
        };
        self.slots[slot]
            .protocol
            .replace_protocol(self.picker.new_resize_protocol(image));
        self.slots[slot].generation = Some(generation);
    }

    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn cancel_pending(&mut self) {
        if let Some(slot) = self.pending.take() {
            self.slots[slot].clear();
        }
    }

    pub(super) fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: Rect,
        current_generation: u64,
    ) {
        if area.is_empty() {
            return;
        }
        if self
            .pending
            .is_some_and(|slot| self.slots[slot].generation != Some(current_generation))
        {
            self.cancel_pending();
        }
        if let Some(slot) = self.pending {
            frame.render_stateful_widget(
                StatefulImage::new(),
                area,
                &mut self.slots[slot].protocol,
            );
            if self.slots[slot].protocol.protocol_type().is_some() {
                self.promote_pending();
                // A second render would overwrite Kitty's one-shot transmission.
                return;
            }
        }
        frame.render_stateful_widget(
            StatefulImage::new(),
            area,
            &mut self.slots[self.front].protocol,
        );
    }

    pub(super) fn update_resize(
        &mut self,
        slot: usize,
        response: ResizeResponse,
        current_generation: u64,
    ) -> bool {
        if self.pending == Some(slot) && self.slots[slot].generation != Some(current_generation) {
            self.cancel_pending();
            return false;
        }
        let accepted = self.slots[slot].protocol.update_resized_protocol(response);
        if accepted && self.pending == Some(slot) {
            self.promote_pending();
        }
        accepted
    }

    pub(super) fn update_pending(&mut self, response: ResizeResponse, current_generation: u64) {
        self.update_resize(1, response, current_generation);
    }

    fn promote_pending(&mut self) {
        if let Some(slot) = self.pending.take() {
            self.slots[self.front].clear();
            self.front = slot;
        }
    }

    pub(super) fn set_background(&mut self, background: Color) {
        let color = match background {
            Color::Rgb(red, green, blue) => Some(Rgba([red, green, blue, 255])),
            _ => None,
        };
        self.background = color;
        self.picker.set_background_color(color);
    }
}

#[cfg(test)]
mod tests;
