use super::{DWELL_SECONDS, MAX_STEP_SECONDS, TRANSITION_SECONDS, Timeline};

fn run(timeline: &mut Timeline, seconds: f32) {
    let steps = (seconds / 0.05).round() as usize;
    for _ in 0..steps {
        timeline.tick(0.05);
    }
}

#[test]
fn auto_advances_after_the_dwell_and_wraps() {
    let mut timeline = Timeline::new(3);
    run(&mut timeline, DWELL_SECONDS - 0.1);
    assert_eq!(timeline.current(), 0);
    run(&mut timeline, 0.2);
    assert_eq!(timeline.current(), 1);
    let transition = timeline.transition().expect("advance animates");
    assert_eq!(transition.from, 0);
    assert!(transition.from_age > DWELL_SECONDS - 0.2);

    run(&mut timeline, TRANSITION_SECONDS + 0.1);
    assert_eq!(timeline.transition(), None);
    run(&mut timeline, 2. * (DWELL_SECONDS + TRANSITION_SECONDS));
    assert_eq!(timeline.current(), 0);
}

#[test]
fn dwell_does_not_run_during_a_transition() {
    let mut timeline = Timeline::new(2);
    timeline.go_to(1, true);
    run(&mut timeline, TRANSITION_SECONDS * 0.5);
    assert_eq!(timeline.dwell_progress(), 0.);
    assert!(timeline.transition().is_some_and(|t| t.progress() > 0.4));
    assert!(timeline.age() > 0.);
}

#[test]
fn hover_pause_freezes_advance_but_not_the_scene_clock() {
    let mut timeline = Timeline::new(2);
    timeline.set_paused(true);
    run(&mut timeline, DWELL_SECONDS * 2.);
    assert_eq!(timeline.current(), 0);
    assert!(timeline.age() > DWELL_SECONDS);
    timeline.set_paused(false);
    run(&mut timeline, DWELL_SECONDS + 0.1);
    assert_eq!(timeline.current(), 1);
}

#[test]
fn jumping_resets_the_clock_and_same_slide_is_a_no_op() {
    let mut timeline = Timeline::new(4);
    run(&mut timeline, 3.);
    timeline.go_to(0, true);
    assert_eq!(timeline.transition(), None);
    assert!(timeline.age() > 2.9);

    timeline.go_to(6, false);
    assert_eq!(timeline.current(), 2);
    assert_eq!(timeline.age(), 0.);
    assert_eq!(timeline.transition(), None);
}

#[test]
fn a_single_slide_replays_itself_through_the_transition() {
    let mut timeline = Timeline::with_dwell(1, 4.);
    run(&mut timeline, 4.1);
    let transition = timeline.transition().expect("replay animates");
    assert_eq!((transition.from, timeline.current()), (0, 0));
    assert!(transition.from_age > 4.);
    assert!(timeline.age() < 0.2);
}

#[test]
fn restart_resets_the_clock_immediately() {
    let mut timeline = Timeline::new(2);
    run(&mut timeline, 2.);
    timeline.restart();
    assert_eq!(timeline.age(), 0.);
    assert_eq!(timeline.dwell_progress(), 0.);
}

#[test]
fn large_frame_gaps_are_clamped() {
    let mut timeline = Timeline::new(2);
    timeline.tick(60.);
    assert_eq!(timeline.age(), MAX_STEP_SECONDS);
    assert_eq!(timeline.current(), 0);
    timeline.tick(-1.);
    assert_eq!(timeline.age(), MAX_STEP_SECONDS);
}
