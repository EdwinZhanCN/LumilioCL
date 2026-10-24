use std::collections::BTreeSet;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use gpui::Context;
use lumilio_core::MapFailure;
use lumilio_core::world_map::Viewport;
use lumilio_map_render::Grid;
use lumilio_plugin_api::map::{TileKey, TileRequest};

use super::{Backend, Command, IN_FLIGHT, MapView, TILE_CAP, stats, worker};

impl MapView {
    /// The world or dimension changed: what it can show is asked for again.
    pub(super) fn context_changed(&mut self) {
        self.selected = None;
        self.marking = false;
        self.routing = false;
        self.route_points.clear();
        self.measure_mode = false;
        self.measure_start = None;
        self.measure_end = None;
        self.annotations.clear();
        self.objects.clear();
        self.objects.layers.clear();
        self.layers_ready = false;
        if let (Some(connection), Some(context)) = (&self.connection, &self.context) {
            let _ = connection.send.try_send(Command::Overlays {
                context: context.clone(),
            });
            let _ = connection.send.try_send(Command::Annotations {
                context: context.clone(),
            });
        }
    }
    pub(super) fn refresh_objects(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        if self.size.contains(&0) {
            return;
        }
        self.objects
            .update(&context, self.view_blocks(), self.camera.scale);
        self.dispatch_objects();
    }
    pub(super) fn dispatch_objects(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        for next in self.objects.next(&context) {
            let sent = self.connection.as_ref().is_some_and(|connection| {
                connection
                    .send
                    .try_send(Command::Objects {
                        generation: next.generation,
                        plugin: next.plugin,
                        request: Box::new(next.request),
                        key: next.key.clone(),
                        cancel: next.cancel,
                    })
                    .is_ok()
            });
            if !sent {
                self.objects.accept(next.generation, &next.key);
                self.objects.fail(next.key, String::new());
            }
        }
    }
    /// The game's files may have changed (the game just exited): the visible
    /// tiles are asked for again, and the host redraws only those whose files
    /// did change; tiles out of view are forgotten.
    pub fn files_changed(&mut self, cx: &mut Context<Self>) {
        let visible: BTreeSet<TileKey> = self.visible.iter().cloned().collect();
        self.tiles.retain(|key, _| visible.contains(key));
        self.stale = visible
            .into_iter()
            .filter(|key| self.tiles.contains_key(key))
            .collect();
        self.failed.clear();
        self.dispatch();
        cx.notify();
    }
    pub(super) fn reset_view(&mut self) {
        self.schedule.update(&[]);
        self.stale.clear();
        self.tiles.clear();
        self.failed.clear();
        self.visible.clear();
        self.serial = self.serial.wrapping_add(1);
        if let Some(old) = self.image.take() {
            self.old.push(old);
        }
    }
    pub(super) fn refresh(&mut self) {
        let started = Instant::now();
        self.refresh_now();
        stats::add(&self.stats.refresh_us, started.elapsed());
        self.stats.refreshes.fetch_add(1, Relaxed);
    }
    pub(super) fn refresh_now(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        let Some((provider, base)) = self.providers.base_maps.get(self.base) else {
            return;
        };
        if base.id == "xaero" && self.xaero_map_ids().len() > 1 && self.xaero_map_choice().is_none()
        {
            self.schedule.update(&[]);
            self.visible.clear();
            self.refresh_objects();
            self.frame();
            return;
        }
        let template = TileKey {
            provider: provider.clone(),
            base_map: if base.id == "xaero" {
                self.xaero_map_choice()
                    .map_or_else(|| base.id.clone(), |id| format!("xaero@{id}"))
            } else {
                base.id.clone()
            },
            world: context.world.clone(),
            dimension: context.dimension.clone(),
            level: self.camera.level(),
            tx: 0,
            tz: 0,
        };
        let visible = Viewport {
            x: self.camera.x,
            z: self.camera.z,
            blocks_per_pixel: self.camera.scale,
            width: self.size[0],
            height: self.size[1],
        }
        .visible(&template);
        self.schedule.update(&visible);
        self.failed.retain(|key, _| visible.contains(key));
        self.stale.retain(|key| visible.contains(key));
        self.visible = visible;
        self.dispatch();
        self.refresh_objects();
        self.frame();
    }
    /// Starts requests for visible tiles that have neither pixels nor a
    /// failure, nearest the centre first, up to [`IN_FLIGHT`] at a time. A
    /// finished tile calls this again, so the queue drains as the map settles.
    pub(super) fn dispatch(&mut self) {
        let Some(context) = self.context.clone() else {
            return;
        };
        let wanted: Vec<TileKey> = self
            .visible
            .iter()
            .filter(|key| {
                (!self.tiles.contains_key(key) || self.stale.contains(key))
                    && !self.failed.contains_key(key)
            })
            .cloned()
            .collect();
        for key in wanted {
            if self.schedule.pending_len() >= IN_FLIGHT {
                break;
            }
            let Some((generation, cancel)) = self.schedule.begin(&key) else {
                continue;
            };
            if let Some(connection) = &self.connection
                && let Err(error) = connection.send.try_send(Command::Tile {
                    generation,
                    request: Box::new(TileRequest {
                        context: context.clone(),
                        key: key.clone(),
                        pixels: 256,
                    }),
                    cancel,
                })
            {
                self.schedule.accept(generation, &key);
                self.failed
                    .insert(key, MapFailure::Failed(error.to_string()));
            }
        }
    }
    /// Drops the least recently drawn tiles beyond [`TILE_CAP`], never one of
    /// the visible ones.
    pub(super) fn evict(&mut self) {
        let extra = self.tiles.len().saturating_sub(TILE_CAP);
        if extra == 0 {
            return;
        }
        let visible: BTreeSet<&TileKey> = self.visible.iter().collect();
        let mut stale: Vec<(u64, TileKey)> = self
            .tiles
            .iter()
            .filter(|(key, _)| !visible.contains(key))
            .map(|(key, tile)| (tile.used, key.clone()))
            .collect();
        stale.sort();
        for (_, key) in stale.into_iter().take(extra) {
            self.tiles.remove(&key);
        }
    }
    /// Hands the render thread what to draw. Costs a handful of `Arc` clones
    /// per tile, so it can run on every pan and tile arrival.
    ///
    /// Draw order, bottom to top: patterns for visible tiles without pixels,
    /// cached tiles of other levels over them (coarsest first) so a zoom never
    /// shows a hole, then the visible level's own tiles.
    pub(super) fn frame(&mut self) {
        // The canvas backend paints from the view's state on every render.
        if self.size.contains(&0) || self.backend == Backend::Canvas {
            return;
        }
        self.serial = self.serial.wrapping_add(1);
        let tiles = self.scene_tiles();
        let sprites = self.sprites();
        if let Some(worker) = &self.worker {
            worker.mailbox.put(worker::Request {
                serial: self.serial,
                issued: Instant::now(),
                camera: lumilio_map_render::Camera {
                    x: self.camera.x,
                    z: self.camera.z,
                    blocks_per_pixel: self.camera.scale,
                },
                tiles,
                sprites,
                grid: Grid {
                    chunks: self.layers.chunks,
                    regions: self.layers.regions,
                },
                width: self.size[0],
                height: self.size[1],
            });
        }
    }
}
