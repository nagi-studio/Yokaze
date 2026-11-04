use super::*;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

struct Display {
    cover: OriginalCover,
    workers: [mpsc::UnboundedReceiver<ResizeRequest>; 2],
    terminal: Terminal<TestBackend>,
}

impl Display {
    fn new() -> Self {
        Self::with_protocol(ratatui_image::picker::ProtocolType::Halfblocks)
    }

    fn with_protocol(protocol: ratatui_image::picker::ProtocolType) -> Self {
        let (main_tx, main_rx) = mpsc::unbounded_channel();
        let (back_tx, back_rx) = mpsc::unbounded_channel();
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(protocol);
        Self {
            cover: OriginalCover::buffered(picker, main_tx, back_tx),
            workers: [main_rx, back_rx],
            terminal: Terminal::new(TestBackend::new(8, 4)).unwrap(),
        }
    }

    fn replace(&mut self, generation: u64, color: [u8; 4]) {
        self.cover.replace(
            generation,
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(80, 80, Rgba(color))),
        );
    }

    fn draw(&mut self, generation: u64) -> Buffer {
        self.terminal
            .draw(|frame| self.cover.render(frame, Rect::new(0, 0, 8, 4), generation))
            .unwrap();
        self.terminal.backend().buffer().clone()
    }

    fn complete(&mut self, slot: usize, generation: u64) {
        let request = self.workers[slot]
            .try_recv()
            .expect("image needs one encoding");
        assert!(self
            .cover
            .update_resize(slot, request.resize_encode().unwrap(), generation));
    }

    fn assert_no_requests(&mut self) {
        for worker in &mut self.workers {
            assert!(matches!(
                worker.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));
        }
    }

    fn show_first(&mut self) -> Buffer {
        self.replace(1, [255, 0, 0, 255]);
        self.draw(1);
        self.complete(0, 1);
        self.draw(1)
    }
}

#[test]
fn handovers_keep_every_frame_and_encode_each_image_once() {
    let mut display = Display::new();
    let mut previous = display.show_first();
    for generation in 2..=6 {
        let color = if generation % 2 == 0 {
            [0, 255, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        display.replace(generation, color);
        assert_eq!(display.draw(generation), previous);
        let slot = 1 - display.cover.front;
        display.complete(slot, generation);
        let current = display.draw(generation);
        assert_ne!(current, previous);
        assert!(current
            .content
            .iter()
            .any(|cell| cell.fg == Color::Rgb(color[0], color[1], color[2])
                || cell.bg == Color::Rgb(color[0], color[1], color[2])));
        assert_eq!(display.cover.generation(), Some(generation));
        assert!(!display.cover.has_pending());
        display.assert_no_requests();
        previous = current;
    }
}

#[test]
fn a_late_b_encoding_cannot_be_promoted_as_c() {
    let mut display = Display::new();
    let first = display.show_first();
    display.replace(2, [0, 255, 0, 255]);
    assert_eq!(display.draw(2), first);
    let stale = display.workers[1].try_recv().unwrap();
    display.replace(3, [0, 0, 255, 255]);
    assert_eq!(display.draw(3), first);
    assert!(!display
        .cover
        .update_resize(1, stale.resize_encode().unwrap(), 3));
    assert_eq!(display.draw(3), first);
    display.complete(1, 3);
    assert_ne!(display.draw(3), first);
    assert_eq!(display.cover.generation(), Some(3));
    display.assert_no_requests();
}

#[test]
fn clear_releases_both_slots_and_rejects_their_in_flight_work_after_reuse() {
    let mut display = Display::new();
    display.show_first();
    display.replace(2, [0, 255, 0, 255]);
    display.draw(2);
    let stale = display.workers[1].try_recv().unwrap();
    display.cover.clear();
    assert!(!display.cover.has_pending());
    assert!(display
        .cover
        .slots
        .iter()
        .all(|slot| slot.generation.is_none() && slot.protocol.protocol_type().is_none()));
    display.replace(3, [255, 0, 0, 255]);
    display.draw(3);
    display.complete(0, 3);
    let third = display.draw(3);
    display.replace(4, [0, 0, 255, 255]);
    assert_eq!(display.draw(4), third);
    assert!(!display
        .cover
        .update_resize(1, stale.resize_encode().unwrap(), 4));
    assert_eq!(display.draw(4), third);
    display.complete(1, 4);
    assert_ne!(display.draw(4), third);
    display.assert_no_requests();
}

#[test]
fn kitty_handover_transmits_each_new_image_before_reusing_its_placeholders() {
    let mut display = Display::with_protocol(ratatui_image::picker::ProtocolType::Kitty);
    let transmission_count = |frame: &Buffer| {
        frame
            .content
            .iter()
            .map(|cell| cell.symbol().matches("a=T,").count())
            .sum::<usize>()
    };
    assert_eq!(transmission_count(&display.show_first()), 1);
    let mut held = display.draw(1);
    for generation in 2..=4 {
        display.replace(generation, [0, (generation * 30) as u8, 255, 255]);
        assert_eq!(display.draw(generation), held);
        let slot = 1 - display.cover.front;
        let response = display.workers[slot]
            .try_recv()
            .unwrap()
            .resize_encode()
            .unwrap();
        if generation == 3 {
            assert!(display.cover.slots[slot]
                .protocol
                .update_resized_protocol(response));
        } else {
            assert!(display.cover.update_resize(slot, response, generation));
        }
        let first = display.draw(generation);
        assert_eq!(transmission_count(&first), 1);
        assert_eq!(display.cover.generation(), Some(generation));
        held = display.draw(generation);
        assert_eq!(transmission_count(&held), 0);
        assert!(held
            .content
            .iter()
            .any(|cell| cell.symbol().contains('\u{10eeee}')));
        display.assert_no_requests();
    }
}
