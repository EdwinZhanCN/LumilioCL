//! Counters for the map's render pipeline: how many frames were asked for,
//! rendered, shown or thrown away, and where the time went. Cheap enough to
//! keep on; `LUMILIO_MAP_STATS=1` prints a summary every two seconds.
//!
//! What it cannot see: GPUI's own upload of a new image into its atlas and the
//! compositor's present happen after `render()` returns and have no hook.
//! `paint_lag` (a result being shown to the next `render()` of the view) is the
//! closest proxy.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

#[derive(Default)]
pub(super) struct Stats {
    /// Frames handed to the render thread.
    pub requested: AtomicU64,
    /// Requests replaced in the mailbox before the render thread took them.
    pub coalesced: AtomicU64,
    /// Frames the render thread finished.
    pub rendered: AtomicU64,
    /// Finished frames that were shown.
    pub displayed: AtomicU64,
    /// Finished frames dropped because a newer request had been made meanwhile.
    pub discarded: AtomicU64,
    pub render_us: AtomicU64,
    pub encode_us: AtomicU64,
    pub wait_us: AtomicU64,
    pub copy_us: AtomicU64,
    /// Request made until the render thread took it.
    pub queue_us: AtomicU64,
    /// Request made until its frame was shown (summed over shown frames).
    pub latency_us: AtomicU64,
    /// Building the `RenderImage` on the UI thread.
    pub image_us: AtomicU64,
    /// Frame shown until the next `render()` of the view (summed).
    pub paint_lag_us: AtomicU64,
    pub paints: AtomicU64,
    /// Main-thread cost of `refresh()` (a pan, zoom or resize).
    pub refresh_us: AtomicU64,
    pub refreshes: AtomicU64,
    pub bytes: AtomicU64,
    /// The GPUI canvas backend: frames painted, CPU time in `paint`, tiles and
    /// sprites drawn, images built.
    pub canvas_paints: AtomicU64,
    pub canvas_us: AtomicU64,
    pub canvas_tiles: AtomicU64,
    pub canvas_sprites: AtomicU64,
    pub canvas_images: AtomicU64,
}

pub(super) fn add(counter: &AtomicU64, by: Duration) {
    counter.fetch_add(by.as_micros() as u64, Relaxed);
}

/// A copy of the counters at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Snapshot {
    pub requested: u64,
    pub coalesced: u64,
    pub rendered: u64,
    pub displayed: u64,
    pub discarded: u64,
    pub render_us: u64,
    pub encode_us: u64,
    pub wait_us: u64,
    pub copy_us: u64,
    pub queue_us: u64,
    pub latency_us: u64,
    pub image_us: u64,
    pub paint_lag_us: u64,
    pub paints: u64,
    pub refresh_us: u64,
    pub refreshes: u64,
    pub bytes: u64,
    pub canvas_paints: u64,
    pub canvas_us: u64,
    pub canvas_tiles: u64,
    pub canvas_sprites: u64,
    pub canvas_images: u64,
}

impl Stats {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            requested: self.requested.load(Relaxed),
            coalesced: self.coalesced.load(Relaxed),
            rendered: self.rendered.load(Relaxed),
            displayed: self.displayed.load(Relaxed),
            discarded: self.discarded.load(Relaxed),
            render_us: self.render_us.load(Relaxed),
            encode_us: self.encode_us.load(Relaxed),
            wait_us: self.wait_us.load(Relaxed),
            copy_us: self.copy_us.load(Relaxed),
            queue_us: self.queue_us.load(Relaxed),
            latency_us: self.latency_us.load(Relaxed),
            image_us: self.image_us.load(Relaxed),
            paint_lag_us: self.paint_lag_us.load(Relaxed),
            paints: self.paints.load(Relaxed),
            refresh_us: self.refresh_us.load(Relaxed),
            refreshes: self.refreshes.load(Relaxed),
            bytes: self.bytes.load(Relaxed),
            canvas_paints: self.canvas_paints.load(Relaxed),
            canvas_us: self.canvas_us.load(Relaxed),
            canvas_tiles: self.canvas_tiles.load(Relaxed),
            canvas_sprites: self.canvas_sprites.load(Relaxed),
            canvas_images: self.canvas_images.load(Relaxed),
        }
    }
}

impl Snapshot {
    pub fn since(self, earlier: Snapshot) -> Snapshot {
        Snapshot {
            requested: self.requested - earlier.requested,
            coalesced: self.coalesced - earlier.coalesced,
            rendered: self.rendered - earlier.rendered,
            displayed: self.displayed - earlier.displayed,
            discarded: self.discarded - earlier.discarded,
            render_us: self.render_us - earlier.render_us,
            encode_us: self.encode_us - earlier.encode_us,
            wait_us: self.wait_us - earlier.wait_us,
            copy_us: self.copy_us - earlier.copy_us,
            queue_us: self.queue_us - earlier.queue_us,
            latency_us: self.latency_us - earlier.latency_us,
            image_us: self.image_us - earlier.image_us,
            paint_lag_us: self.paint_lag_us - earlier.paint_lag_us,
            paints: self.paints - earlier.paints,
            refresh_us: self.refresh_us - earlier.refresh_us,
            refreshes: self.refreshes - earlier.refreshes,
            bytes: self.bytes - earlier.bytes,
            canvas_paints: self.canvas_paints - earlier.canvas_paints,
            canvas_us: self.canvas_us - earlier.canvas_us,
            canvas_tiles: self.canvas_tiles - earlier.canvas_tiles,
            canvas_sprites: self.canvas_sprites - earlier.canvas_sprites,
            canvas_images: self.canvas_images - earlier.canvas_images,
        }
    }

    /// Share of finished frames that were thrown away, 0 to 1.
    pub fn discard_ratio(&self) -> f64 {
        let finished = self.displayed + self.discarded;
        if finished == 0 {
            0.
        } else {
            self.discarded as f64 / finished as f64
        }
    }

    /// One line: counts, then per-event averages in milliseconds.
    pub fn line(&self) -> String {
        if self.canvas_paints > 0 {
            let per = |n: u64| n as f64 / self.canvas_paints as f64;
            return format!(
                "canvas: {} paints, {:.3} ms CPU each ({:.0} tiles, {:.0} sprites, {} new images) | \
                 refresh {:.3} ms x{}",
                self.canvas_paints,
                per(self.canvas_us) / 1000.,
                per(self.canvas_tiles),
                per(self.canvas_sprites),
                self.canvas_images,
                if self.refreshes == 0 {
                    0.
                } else {
                    self.refresh_us as f64 / self.refreshes as f64 / 1000.
                },
                self.refreshes,
            );
        }
        let ms = |us: u64, n: u64| {
            if n == 0 {
                0.
            } else {
                us as f64 / n as f64 / 1000.
            }
        };
        format!(
            "requested {} (coalesced {}) rendered {} shown {} discarded {} ({:.0}%) | \
             render {:.2} ms [encode {:.2} wait {:.2} copy {:.2}] queue {:.2} | \
             request->shown {:.2} ms, image {:.2} ms, shown->paint {:.2} ms, refresh {:.3} ms x{} | \
             {:.1} MB/frame",
            self.requested,
            self.coalesced,
            self.rendered,
            self.displayed,
            self.discarded,
            self.discard_ratio() * 100.,
            ms(self.render_us, self.rendered),
            ms(self.encode_us, self.rendered),
            ms(self.wait_us, self.rendered),
            ms(self.copy_us, self.rendered),
            ms(self.queue_us, self.rendered),
            ms(self.latency_us, self.displayed),
            ms(self.image_us, self.displayed),
            ms(self.paint_lag_us, self.paints),
            ms(self.refresh_us, self.refreshes),
            self.refreshes,
            if self.rendered == 0 {
                0.
            } else {
                self.bytes as f64 / self.rendered as f64 / 1.0e6
            },
        )
    }
}
