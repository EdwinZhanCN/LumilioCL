//! A measurement of the render pipeline under a simulated drag, not a
//! pass/fail test: `cargo nextest run -p lumilio-ui drag_pipeline --ignored --no-capture`.
//! `LUMILIO_PERF_RATIO` sets the pixel ratio (1 = the logical-size frames the
//! map used before HiDPI, 2 = a Retina frame); `LUMILIO_PERF_HZ` the event rate.
use super::stats::Snapshot;
use super::worker::{Request, Worker};
use lumilio_map_render::{Camera, Grid, Sprite, Tile};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

fn env(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn tiles() -> Vec<Tile> {
    // 7 x 5 level-1 tiles (1024 blocks each) around the origin, each unique so
    // the texture cache holds them as it would a loaded map.
    (-3..4)
        .flat_map(|tx| (-2..3).map(move |tz| (tx, tz)))
        .map(|(tx, tz)| Tile {
            id: format!("perf-{tx}-{tz}"),
            x: f64::from(tx) * 1024.,
            z: f64::from(tz) * 1024.,
            span: 1024.,
            rgba: (0..256 * 256)
                .flat_map(|at| {
                    let shade = ((at * 7 + tx * 31 + tz * 17) % 200) as u8;
                    [shade, 255 - shade, shade / 2, 255]
                })
                .collect::<Vec<u8>>()
                .into(),
        })
        .collect()
}

fn sprites(ratio: f64, width: f64, height: f64) -> Vec<Sprite> {
    let icon: Arc<[u8]> = (0..80 * 80)
        .flat_map(|at| {
            [
                200,
                (at % 255) as u8,
                60,
                if at % 80 < 70 { 255 } else { 0 },
            ]
        })
        .collect::<Vec<u8>>()
        .into();
    (0..150)
        .map(|n| Sprite {
            id: "perf".into(),
            width: 80,
            height: 80,
            rgba: icon.clone(),
            x: f64::from(n % 15) / 15. * width + 20.,
            y: f64::from(n / 15) / 10. * height + 20.,
            size: (28. * ratio).round() as u32,
            opacity: 1.,
        })
        .collect()
}

#[test]
#[ignore = "measurement, prints a report"]
fn drag_pipeline_measurement() {
    let ratio = env("LUMILIO_PERF_RATIO", 1.);
    let hz = env("LUMILIO_PERF_HZ", 120.);
    let (logical_w, logical_h) = (1000., 700.);
    let (width, height) = (
        (logical_w * ratio).round() as u32,
        (logical_h * ratio).round() as u32,
    );
    let began = Instant::now();
    let worker = Worker::start().unwrap();
    let (first_send, first) = std::sync::mpsc::channel();
    let latest = Arc::new(AtomicU64::new(0));
    let (displayed, discarded) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    let output = worker.output.clone();
    let counted = (latest.clone(), displayed.clone(), discarded.clone());
    let stats = worker.stats.clone();
    // The UI thread's part: show a result only if it answers the newest request.
    let consumer = std::thread::spawn(move || {
        let mut told = false;
        while let Ok(result) = output.recv_blocking() {
            let done = result.unwrap();
            if !std::mem::replace(&mut told, true) {
                let _ = first_send.send(());
            }
            if done.serial == counted.0.load(Relaxed) {
                counted.1.fetch_add(1, Relaxed);
                super::stats::add(&stats.latency_us, done.issued.elapsed());
            } else {
                counted.2.fetch_add(1, Relaxed);
            }
        }
    });
    let (tiles, sprites) = (tiles(), sprites(ratio, f64::from(width), f64::from(height)));
    let started = Instant::now();
    let mut serial = 0;
    let mut send = |at: f64| {
        serial += 1;
        latest.store(serial, Relaxed);
        worker.mailbox.put(Request {
            serial,
            issued: Instant::now(),
            camera: Camera {
                x: at * 4.,
                z: at,
                blocks_per_pixel: 4. / ratio,
            },
            tiles: tiles.clone(),
            sprites: sprites.clone(),
            grid: Grid::default(),
            width,
            height,
        });
    };
    // The render thread builds its device and pipelines before the first frame;
    // wait for that, and report it: it is the map's start-up delay.
    send(0.);
    first
        .recv_timeout(Duration::from_secs(180))
        .expect("the first frame");
    println!(
        "first frame after {:.2}s (device, pipelines, texture uploads)",
        began.elapsed().as_secs_f64()
    );
    // Warm up: more frames so every texture is resident and caches are hot.
    for step in 1..30 {
        send(f64::from(step));
        std::thread::sleep(Duration::from_millis(40));
    }
    let _ = started;
    let before = worker.stats.snapshot();
    let (shown0, dropped0) = (displayed.load(Relaxed), discarded.load(Relaxed));
    let events = (hz * 4.) as u32;
    let step = Duration::from_secs_f64(1. / hz);
    let run = Instant::now();
    for n in 0..events {
        send(30. + f64::from(n) * 0.5);
        let due = run + step * (n + 1);
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut report = worker.stats.snapshot().since(before);
    report.displayed = displayed.load(Relaxed) - shown0;
    report.discarded = discarded.load(Relaxed) - dropped0;
    println!(
        "ratio {ratio} -> {width}x{height} px, {hz} Hz for {:.1}s, {} tiles, {} sprites",
        run.elapsed().as_secs_f64() - 0.3,
        tiles.len(),
        sprites.len()
    );
    println!("{}", Snapshot::line(&report));
    drop(worker);
    let _ = consumer.join();
}
