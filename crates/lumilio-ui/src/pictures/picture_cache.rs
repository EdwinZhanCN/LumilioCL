use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, Entity, EntityId, Global, ImageCache, ImageCacheElement,
    ImageCacheError, RenderImage, Resource, WeakEntity, Window, WindowId, image_cache,
};

use super::decode;
use super::decode::Limits;

/// Decoded bytes the page pictures may hold beyond what is on screen.
pub(super) const PAGE_BUDGET: usize = 64 << 20;
/// Nothing inside a page is drawn wider than a dialog on a 2x display.
pub(super) const PAGE_EDGE: u32 = 2048;
/// The wallpaper fills the window, up to a 4K display.
pub(super) const WALLPAPER_EDGE: u32 = 3840;
/// An animation larger than this decoded shows its first frame only.
pub(super) const ANIMATION_BYTES: usize = 32 << 20;

/// The cache for pictures inside pages and dialogs.
pub fn pages(cx: &mut App) -> Entity<PictureCache> {
    caches(cx).pages.clone()
}

/// The cache for the Home wallpaper: only the one on screen is kept, at a
/// size that fills the window.
pub fn wallpaper(cx: &mut App) -> Entity<PictureCache> {
    caches(cx).wallpaper.clone()
}

/// Makes every picture loaded by path or address inside it use [`pages`].
pub fn scope(cx: &mut App) -> ImageCacheElement {
    image_cache(pages(cx))
}

/// Lets go of every picture not on screen, whatever the budget allows: a game
/// has started and has better use for the memory. Pictures shown again load
/// again.
pub fn release_unseen(cx: &mut App) {
    let Some(caches) = cx.try_global::<Caches>() else {
        return;
    };
    let caches = [caches.pages.clone(), caches.wallpaper.clone()];
    // Outside any window update, so every window's atlas is reached.
    cx.defer(move |cx| {
        for cache in caches {
            cache.update(cx, |cache, cx| cache.evict_beyond(0, None, cx));
        }
    });
}

pub(super) struct Caches {
    pub(super) pages: Entity<PictureCache>,
    pub(super) wallpaper: Entity<PictureCache>,
}

impl Global for Caches {}

pub(super) fn caches(cx: &mut App) -> &Caches {
    if !cx.has_global::<Caches>() {
        let animation = ANIMATION_BYTES;
        let caches = Caches {
            pages: PictureCache::new(
                Limits {
                    edge: PAGE_EDGE,
                    animation,
                },
                PAGE_BUDGET,
                cx,
            ),
            wallpaper: PictureCache::new(
                Limits {
                    edge: WALLPAPER_EDGE,
                    animation,
                },
                0,
                cx,
            ),
        };
        cx.set_global(caches);
    }
    cx.global::<Caches>()
}

pub struct PictureCache {
    pub(super) this: WeakEntity<Self>,
    pub(super) limits: Limits,
    pub(super) budget: usize,
    pub(super) entries: HashMap<Resource, Entry>,
    /// Decoded bytes of the loaded pictures.
    pub(super) held: usize,
    /// Orders entries by when they were last asked for.
    pub(super) clock: u64,
    pub(super) windows: HashMap<WindowId, Draws>,
}

pub(super) struct Entry {
    pub(super) state: State,
    /// The latest frame of each window that asked for it.
    pub(super) seen: Vec<(WindowId, u64)>,
    pub(super) used: u64,
}

pub(super) enum State {
    /// Views to redraw once the picture is there.
    Loading(Vec<EntityId>),
    Loaded(Result<Arc<RenderImage>, ImageCacheError>),
}

/// A window's frames, as this cache counts them.
#[derive(Default)]
pub(super) struct Draws {
    /// The frame being drawn now, or next.
    pub(super) current: u64,
    /// Whether the end of the current frame is already awaited.
    pub(super) ticking: bool,
}

impl PictureCache {
    pub(super) fn new(limits: Limits, budget: usize, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            this: cx.weak_entity(),
            limits,
            budget,
            entries: HashMap::new(),
            held: 0,
            clock: 0,
            windows: HashMap::new(),
        })
    }

    /// Decoded bytes of the pictures held.
    pub fn held(&self) -> usize {
        self.held
    }

    /// Whether `resource` is loaded or loading.
    pub fn contains(&self, resource: &Resource) -> bool {
        self.entries.contains_key(resource)
    }

    /// The frame `window` is drawing, making sure its end is noticed.
    pub(super) fn frame(&mut self, window: &Window) -> (WindowId, u64) {
        let id = window.window_handle().window_id();
        let draws = self.windows.entry(id).or_default();
        if !draws.ticking {
            draws.ticking = true;
            let this = self.this.clone();
            window.on_next_frame(move |window, cx| {
                let _ = this.update(cx, |cache, cx| cache.frame_ended(id, window, cx));
            });
        }
        (id, draws.current)
    }

    pub(super) fn frame_ended(
        &mut self,
        id: WindowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(draws) = self.windows.get_mut(&id) {
            draws.current += 1;
            draws.ticking = false;
        }
        self.evict(Some(window), cx);
    }

    /// Reads and decodes `resource` off the interface thread.
    pub(super) fn start(&self, resource: Resource, cx: &mut App) {
        let (client, assets, svg) = (
            cx.http_client(),
            cx.asset_source().clone(),
            cx.svg_renderer(),
        );
        let limits = self.limits;
        let source = resource.clone();
        let work = cx.background_spawn(async move {
            let bytes = decode::read(source, client, assets).await?;
            decode::decode(&bytes, limits, &svg)
        });
        let this = self.this.clone();
        cx.spawn(async move |cx| {
            let result = work.await;
            let _ = this.update(cx, |cache, cx| cache.loaded(&resource, result, cx));
        })
        .detach();
    }

    pub(super) fn loaded(
        &mut self,
        resource: &Resource,
        result: Result<Arc<RenderImage>, ImageCacheError>,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.entries.get_mut(resource) else {
            return;
        };
        if let Ok(picture) = &result {
            self.held += bytes(picture);
        }
        if let State::Loading(waiting) = std::mem::replace(&mut entry.state, State::Loaded(result))
        {
            let app: &mut App = cx;
            for view in waiting {
                app.notify(view);
            }
        }
        self.evict(None, cx);
    }

    pub(super) fn evict(&mut self, window: Option<&mut Window>, cx: &mut App) {
        self.evict_beyond(self.budget, window, cx);
    }

    /// Lets go of what no window shows: failed pictures, so they are tried
    /// again when next shown, and the least recently used pictures while more
    /// than `budget` is held. `window` is the one being updated, if any.
    pub(super) fn evict_beyond(
        &mut self,
        budget: usize,
        mut window: Option<&mut Window>,
        cx: &mut App,
    ) {
        let open: HashSet<WindowId> = cx.windows().iter().map(|w| w.window_id()).collect();
        self.windows.retain(|id, _| open.contains(id));
        let windows = &self.windows;
        // What a window's latest frame asked for is on screen; while that
        // frame is being drawn, so is what the one before it asked for.
        let shown = |entry: &Entry| {
            entry.seen.iter().any(|(id, frame)| {
                windows
                    .get(id)
                    .is_some_and(|draws| frame + 1 >= draws.current)
            })
        };
        let mut idle: Vec<(u64, Resource)> = self
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry.state, State::Loaded(_)) && !shown(entry))
            .map(|(resource, entry)| (entry.used, resource.clone()))
            .collect();
        idle.sort_unstable_by_key(|(used, _)| *used);
        for (_, resource) in idle {
            let failed = matches!(self.entries[&resource].state, State::Loaded(Err(_)));
            if !failed && self.held <= budget {
                continue;
            }
            if let Some(Entry {
                state: State::Loaded(Ok(picture)),
                ..
            }) = self.entries.remove(&resource)
            {
                self.held -= bytes(&picture);
                cx.drop_image(picture, window.as_deref_mut());
            }
        }
    }
}

impl ImageCache for PictureCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let (id, frame) = self.frame(window);
        self.clock += 1;
        if !self.entries.contains_key(resource) {
            self.start(resource.clone(), cx);
        }
        let entry = self.entries.entry(resource.clone()).or_insert(Entry {
            state: State::Loading(Vec::new()),
            seen: Vec::new(),
            used: 0,
        });
        entry.used = self.clock;
        match entry.seen.iter_mut().find(|(seen, _)| *seen == id) {
            Some(seen) => seen.1 = frame,
            None => entry.seen.push((id, frame)),
        }
        match &mut entry.state {
            State::Loading(waiting) => {
                let view = window.current_view();
                if !waiting.contains(&view) {
                    waiting.push(view);
                }
                None
            }
            State::Loaded(result) => Some(result.clone()),
        }
    }
}

/// Decoded bytes of every frame of `picture`.
pub(super) fn bytes(picture: &RenderImage) -> usize {
    (0..picture.frame_count())
        .filter_map(|frame| picture.as_bytes(frame))
        .map(<[u8]>::len)
        .sum()
}
