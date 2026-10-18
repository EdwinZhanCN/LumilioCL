use super::decode::{self, Limits};
use super::*;
use gpui::{
    BackgroundExecutor, Context, IntoElement, ParentElement as _, Render, Styled as _, SvgRenderer,
    TestAppContext, VisualTestContext, img, px,
};
use std::path::{Path, PathBuf};

const LIMITS: Limits = Limits {
    edge: 2048,
    animation: 1 << 20,
};

fn png(path: &Path, width: u32, height: u32) {
    image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]))
        .save(path)
        .unwrap();
}

fn svg() -> SvgRenderer {
    SvgRenderer::new(std::sync::Arc::new(()))
}

#[test]
fn a_picture_is_decoded_no_larger_than_the_edge() {
    let dir = tempfile::tempdir().unwrap();
    // A PNG is shrunk as it is read; a JPEG is decoded whole, then shrunk.
    let wide_png = dir.path().join("wide.png");
    png(&wide_png, 3000, 1000);
    let wide_jpeg = dir.path().join("wide.jpg");
    image::RgbImage::from_pixel(3000, 1000, image::Rgb([255, 0, 0]))
        .save(&wide_jpeg)
        .unwrap();
    for path in [wide_png, wide_jpeg] {
        let picture = decode::decode(&std::fs::read(&path).unwrap(), LIMITS, &svg()).unwrap();
        assert_eq!(picture.frame_count(), 1);
        let size = picture.size(0);
        assert_eq!((size.width.0, size.height.0), (2048, 683), "{path:?}");
        let [blue, green, red, alpha] = picture.as_bytes(0).unwrap()[..4] else {
            unreachable!("a pixel is four bytes")
        };
        assert!(
            red > 240 && green < 16 && blue < 16 && alpha == 255,
            "red, blue first as GPUI's atlas expects: {path:?}"
        );
    }
}

fn gif(frames: usize, edge: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for index in 0..frames {
            let shade = (index * 40) as u8;
            let buffer = image::RgbaImage::from_pixel(edge, edge, image::Rgba([shade, 0, 0, 255]));
            encoder.encode_frame(image::Frame::new(buffer)).unwrap();
        }
    }
    bytes
}

#[test]
fn an_animation_over_its_limit_keeps_only_its_first_frame() {
    // 64x64 frames are 16 KiB each.
    let small = Limits {
        edge: 2048,
        animation: 100 << 10,
    };
    let within = decode::decode(&gif(4, 64), small, &svg()).unwrap();
    assert_eq!(within.frame_count(), 4, "64 KiB fits");
    let over = decode::decode(&gif(8, 64), small, &svg()).unwrap();
    assert_eq!(over.frame_count(), 1, "128 KiB does not");
}

struct Wall {
    cache: Entity<PictureCache>,
    shown: Vec<PathBuf>,
}

impl Render for Wall {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        image_cache(self.cache.clone()).size_full().children(
            self.shown
                .iter()
                .map(|path| img(path.clone()).size(px(10.))),
        )
    }
}

fn resource(path: &Path) -> Resource {
    Resource::Path(path.into())
}

fn picture(
    cache: &Entity<PictureCache>,
    path: &Path,
    cx: &mut VisualTestContext,
) -> Arc<RenderImage> {
    cx.update(
        |_, cx| match &cache.read(cx).entries[&resource(path)].state {
            State::Loaded(Ok(picture)) => picture.clone(),
            _ => panic!("{path:?} is not loaded"),
        },
    )
}

/// A window showing `shown`. Unlike `add_window_view` it does not run the
/// executor until parked, which a cache reloading what it shows never is.
fn open(
    cache: &Entity<PictureCache>,
    shown: Vec<PathBuf>,
    cx: &mut TestAppContext,
) -> (Entity<Wall>, VisualTestContext) {
    let cache = cache.clone();
    let window = cx.add_window(|_, _| Wall { cache, shown });
    let wall = window.root(cx).unwrap();
    (wall, VisualTestContext::from_window(window.into(), cx))
}

/// Lets the loads finish, the view redraw and the frame end.
fn settle(executor: &BackgroundExecutor, cx: &mut VisualTestContext) {
    for _ in 0..3 {
        park(executor);
        cx.update(|window, cx| window.simulate_next_frame(cx));
    }
    park(executor);
}

/// Runs what is ready, failing rather than spinning when pictures on screen
/// keep being loaded again.
fn park(executor: &BackgroundExecutor) {
    for _ in 0..500 {
        if !executor.tick() {
            return;
        }
    }
    panic!("still busy: pictures on screen keep being loaded again");
}

#[gpui::test]
fn pictures_off_screen_are_let_go_past_the_budget_and_leave_the_atlas(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = ["a", "b", "c", "d"]
        .iter()
        .map(|name| {
            let path = dir.path().join(format!("{name}.png"));
            png(&path, 64, 64);
            path
        })
        .collect();
    let [a, b, c, d] = [&paths[0], &paths[1], &paths[2], &paths[3]];
    // 64x64 pictures are 16 KiB each: two and a half fit.
    let executor = cx.executor();
    let cache = cx.update(|cx| PictureCache::new(LIMITS, 40 << 10, cx));
    let (wall, mut cx) = open(&cache, vec![a.clone(), b.clone(), c.clone()], cx);
    let cx = &mut cx;
    settle(&executor, cx);
    assert_eq!(
        cache.read_with(cx, |cache, _| cache.held()),
        48 << 10,
        "a page over the budget keeps what it shows"
    );
    let (first, second, third) = (
        picture(&cache, a, cx),
        picture(&cache, b, cx),
        picture(&cache, c, cx),
    );
    assert!(cx.update(|window, _| window.has_image_atlas_entry(&first)));

    wall.update(cx, |wall, cx| {
        wall.shown = vec![d.clone()];
        cx.notify();
    });
    settle(&executor, cx);
    let fourth = picture(&cache, d, cx);
    cache.read_with(cx, |cache, _| {
        assert!(!cache.contains(&resource(a)) && !cache.contains(&resource(b)));
        assert!(
            cache.contains(&resource(c)),
            "the most recent fits the budget"
        );
        assert_eq!(cache.held(), 32 << 10);
    });
    cx.update(|window, _| {
        assert!(!window.has_image_atlas_entry(&first), "texture returned");
        assert!(!window.has_image_atlas_entry(&second));
        assert!(window.has_image_atlas_entry(&third));
        assert!(
            window.has_image_atlas_entry(&fourth),
            "drawn once it arrived"
        );
    });
}

#[gpui::test]
fn a_picture_that_failed_is_tried_again_once_it_returns_to_the_screen(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (missing, other) = (dir.path().join("late.png"), dir.path().join("other.png"));
    png(&other, 8, 8);
    let executor = cx.executor();
    let cache = cx.update(|cx| PictureCache::new(LIMITS, 1 << 20, cx));
    let (wall, mut cx) = open(&cache, vec![missing.clone()], cx);
    let cx = &mut cx;
    settle(&executor, cx);
    assert!(cache.read_with(cx, |cache, _| cache.contains(&resource(&missing))));

    wall.update(cx, |wall, cx| {
        wall.shown = vec![other.clone()];
        cx.notify();
    });
    settle(&executor, cx);
    assert!(
        !cache.read_with(cx, |cache, _| cache.contains(&resource(&missing))),
        "a failure is not kept once off screen"
    );

    png(&missing, 8, 8);
    wall.update(cx, |wall, cx| {
        wall.shown = vec![missing.clone()];
        cx.notify();
    });
    settle(&executor, cx);
    picture(&cache, &missing, cx);
}

#[gpui::test]
fn a_game_starting_lets_go_of_every_picture_off_screen(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = ["a", "b", "c"]
        .iter()
        .map(|name| {
            let path = dir.path().join(format!("{name}.png"));
            png(&path, 64, 64);
            path
        })
        .collect();
    let [a, b, c] = [&paths[0], &paths[1], &paths[2]];
    let executor = cx.executor();
    let cache = cx.update(pages);
    let (wall, mut cx) = open(&cache, vec![a.clone(), b.clone()], cx);
    let cx = &mut cx;
    settle(&executor, cx);
    let first = picture(&cache, a, cx);
    wall.update(cx, |wall, cx| {
        wall.shown = vec![c.clone()];
        cx.notify();
    });
    settle(&executor, cx);
    assert_eq!(
        cache.read_with(cx, |cache, _| cache.held()),
        48 << 10,
        "well within the budget, so nothing went"
    );

    cx.update(|_, cx| release_unseen(cx));
    settle(&executor, cx);
    cache.read_with(cx, |cache, _| {
        assert!(!cache.contains(&resource(a)) && !cache.contains(&resource(b)));
        assert!(cache.contains(&resource(c)), "what is on screen stays");
        assert_eq!(cache.held(), 16 << 10);
    });
    assert!(!cx.update(|window, _| window.has_image_atlas_entry(&first)));
}
