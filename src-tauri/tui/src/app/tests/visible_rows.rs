use super::*;

fn visible(state: &AppState, rows: &[SongRow]) -> Vec<(usize, i64)> {
    state
        .visible_rows(rows)
        .iter()
        .map(|(index, row)| (index.to_owned(), row.id))
        .collect()
}

#[test]
fn row_views_preserve_filter_order_ordinals_and_direct_window_access() {
    let mut state = AppState::new(&Config::default());
    state.library = (0..20_000).map(row).collect();
    let rows = state.visible_rows(&state.library);
    assert_eq!(
        rows.get(19_999).map(|(index, row)| (index, row.id)),
        Some((19_999, 19_999))
    );
    assert_eq!(
        rows.iter()
            .skip(19_995)
            .take(3)
            .map(|(_, row)| row.id)
            .collect::<Vec<_>>(),
        [19_995, 19_996, 19_997]
    );

    state.library = vec![
        named_row(1, "夜の窓", "Fishmans"),
        named_row(2, "Night Cruising", "FISHMANS"),
        named_row(3, "İstanbul", "One"),
    ];
    for query in ["", "\u{2003}\n ", "ngtfsh", "夜fish", "İ", "fish night"] {
        state.filter.query = query.into();
        let expected: Vec<_> = state
            .library
            .iter()
            .enumerate()
            .filter(|(_, row)| state.filter.matches(row))
            .map(|(index, row)| (index, row.id))
            .collect();
        assert_eq!(visible(&state, &state.library), expected, "{query:?}");
        assert_eq!(
            state
                .visible_rows(&state.library)
                .last()
                .map(|(index, row)| (index, row.id)),
            expected.last().copied()
        );
    }
}
