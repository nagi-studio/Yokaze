use super::*;

fn draw(state: &mut AppState, width: u16, height: u16, hits: &mut ui::Hits) {
    state.terminal_size = (width, height);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::draw(frame, state, hits)).unwrap();
}

#[test]
fn spectrum_visibility_follows_the_rendered_view_and_geometry() {
    let mut state = AppState::new(&Config::default());
    state.now = Some(NowPlaying {
        title: "Track".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        artist_id: None,
        album_id: None,
    });
    state.config.spectrum_enabled = true;
    let mut hits = ui::Hits::default();
    draw(&mut state, 120, 40, &mut hits);
    assert!(hits.spectrum.is_some());
    state.view = View::Library;
    draw(&mut state, 120, 40, &mut hits);
    assert!(hits.spectrum.is_none());
    state.zen = true;
    draw(&mut state, 120, 40, &mut hits);
    assert!(hits.spectrum.is_some());
    draw(&mut state, 120, 6, &mut hits);
    assert!(hits.spectrum.is_none());
    state.zen = false;
    state.view = View::NowPlaying;
    state.config.spectrum_enabled = false;
    draw(&mut state, 120, 40, &mut hits);
    assert!(hits.spectrum.is_none());
}

#[test]
fn settings_visibility_requires_an_actual_spectrum_preview() {
    use crate::app::settings::SettingField;

    let mut state = AppState::new(&Config::default());
    state.view = View::Settings;
    let mut hits = ui::Hits::default();
    for (index, setting) in SettingField::ALL.iter().enumerate() {
        state.settings.selected = index;
        draw(&mut state, 120, 40, &mut hits);
        let spectrum_setting = matches!(
            setting,
            SettingField::SpectrumEnabled
                | SettingField::SpectrumStyle
                | SettingField::SpectrumGlow
                | SettingField::SpectrumGradient
                | SettingField::SpectrumBars
                | SettingField::SpectrumSensitivity
                | SettingField::SpectrumFlatten
                | SettingField::SpectrumStereo
                | SettingField::SpectrumDb
        );
        assert_eq!(hits.spectrum.is_some(), spectrum_setting, "{setting:?}");
    }
}
