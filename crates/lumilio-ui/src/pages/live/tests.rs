#[test]
fn the_record_reads_hours_minutes_and_counts() {
    use super::record_readings;
    use crate::live::HomePlaces;
    let places = |play_seconds, worlds, servers| HomePlaces {
        instance: "a".to_owned(),
        places: Vec::new(),
        play_seconds,
        worlds,
        servers,
    };
    let fresh: Vec<_> = record_readings(&places(0, 0, 0))
        .iter()
        .map(|reading| (reading.value, reading.unit))
        .collect();
    assert_eq!(
        fresh,
        [(0, Some("MIN")), (0, None), (0, None)],
        "a new game reads zeros instead of hiding the record"
    );
    let long = record_readings(&places(128 * 3600 + 59, 3, 1));
    let shown: Vec<_> = long
        .iter()
        .map(|reading| (reading.label.as_ref(), reading.value, reading.unit))
        .collect();
    assert_eq!(
        shown,
        [
            ("游玩时间", 128, Some("H")),
            ("世界", 3, None),
            ("服务器", 1, None)
        ]
    );
    let short = record_readings(&places(25 * 60, 0, 0));
    assert_eq!((short[0].value, short[0].unit), (25, Some("MIN")));
}
