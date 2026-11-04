use super::*;
use crate::yrc::parse_yrc;

fn assert_alignment(lrc: &str, yrc: &str, expected: &[Option<usize>]) {
    let words = parse_yrc(yrc);
    let lines = parse_with_yrc(lrc, None, &words);
    let actual: Vec<_> = lines.into_iter().map(|line| line.word_timing).collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|index| index.map(|index| words[index].clone()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn equal_scores_prefer_match_then_skip_lrc_then_skip_yrc() {
    assert_alignment(
        "[00:01]啊\n[00:02]啊",
        "[1500,100](1500,100,0)啊",
        &[Some(0), None],
    );
    assert_alignment(
        "[00:01.500]啊",
        "[1000,100](1000,100,0)啊\n[2000,200](2000,200,0)啊",
        &[Some(0)],
    );
    assert_alignment(
        "[00:01]A\n[00:01]B",
        "[1000,100](1000,100,0)B\n[1000,200](1000,200,0)A",
        &[None, Some(0)],
    );
}

#[test]
fn blank_lines_and_partial_word_timings_do_not_shift_later_matches() {
    assert_alignment(
        "[00:01]intro\n[00:02]\n[00:03]verse\n[00:04]\n[00:05]end",
        "[1000,100](1000,100,0)intro\n[2050,100](2050,100,0)interlude\n[3000,100](3000,100,0)verse\n[4200,100](4200,100,0)interlude\n[5000,100](5000,100,0)end",
        &[Some(0), Some(1), Some(2), None, Some(4)],
    );
}

#[test]
fn very_different_lengths_preserve_both_end_matches() {
    let lrc = (0..257)
        .map(|index| format!("[{index}:00]line-{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut expected = vec![None; 257];
    expected[0] = Some(0);
    expected[256] = Some(1);
    assert_alignment(
        &lrc,
        "[0,100](0,100,0)line-0\n[15360000,100](15360000,100,0)line-256",
        &expected,
    );

    let yrc = (0..257)
        .map(|index| {
            format!(
                "[{},100]({},100,0)line-{index}",
                index * 60_000,
                index * 60_000
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_alignment(
        "[00:00]line-0\n[256:00]line-256",
        &yrc,
        &[Some(0), Some(256)],
    );
}

#[test]
fn absent_word_timings_preserve_lrc_and_translations() {
    let lrc = "[00:01]夜の窓\n[00:02]\n[00:03]灯影";
    let translation = Some("[00:01]Window\n[00:03]Light");
    assert_eq!(
        parse_with_yrc(lrc, translation, &[]),
        parse_lrc(lrc, translation)
    );
}
