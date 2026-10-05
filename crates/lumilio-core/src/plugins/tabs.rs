//! Instance tabs contributed by plugins: which appear, what they show, and
//! what their actions ask the host to do.

use std::path::PathBuf;
use std::sync::Arc;

use lumilio_plugin_api::{ActionId, Effect, GameFacts, PluginError, TabState, View};

use super::{PluginHost, PluginStatus, access};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginTab {
    pub plugin: String,
    pub title: String,
}

/// An [`Effect`] after the host checked it; ready for the interface to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginEffect {
    Reveal(PathBuf),
    SaveAs {
        suggested_name: String,
        bytes: Vec<u8>,
    },
    Toast(String),
}

impl PluginHost {
    /// The tabs that show for an instance with these facts, in plugin order.
    pub async fn tabs(&self, game_dir: PathBuf, game: GameFacts) -> Vec<PluginTab> {
        let game = Arc::new(game);
        let mut tabs = Vec::new();
        for info in self.list().await {
            if info.status != PluginStatus::Enabled {
                continue;
            }
            let game = game.clone();
            let shown = self
                .call_in(
                    &info.manifest.id,
                    Some(game_dir.clone()),
                    move |plugin, ctx| {
                        Ok(plugin
                            .instance_tab()
                            .filter(|tab| tab.appears(&game, ctx))
                            .map(|tab| tab.title()))
                    },
                )
                .await
                .flatten();
            if let Some(title) = shown {
                tabs.push(PluginTab {
                    plugin: info.manifest.id,
                    title,
                });
            }
        }
        // Preferences may have changed while a plugin was running.
        let enabled: Vec<_> = self
            .list()
            .await
            .into_iter()
            .filter(|info| info.status == PluginStatus::Enabled)
            .map(|info| info.manifest.id)
            .collect();
        tabs.retain(|tab| enabled.contains(&tab.plugin));
        tabs
    }

    fn tab_state(&self, instance: &str, plugin: &str) -> TabState {
        self.tab_states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(instance.to_owned(), plugin.to_owned()))
            .cloned()
            .unwrap_or(TabState::Null)
    }

    /// What the plugin's tab shows now; `None` when it is off or failed.
    pub async fn tab_view(&self, instance: &str, game_dir: PathBuf, plugin: &str) -> Option<View> {
        let state = self.tab_state(instance, plugin);
        self.call_in(plugin, Some(game_dir), move |plugin, ctx| {
            plugin
                .instance_tab()
                .ok_or_else(|| PluginError::Unavailable("plugin has no instance tab".into()))?
                .view(ctx, &state)
        })
        .await
    }

    /// Runs one action: saves the plugin's new state and returns the effects
    /// that passed the host's checks. `None` when the plugin is off or failed.
    pub async fn tab_action(
        &self,
        instance: &str,
        game_dir: PathBuf,
        plugin: &str,
        action: ActionId,
    ) -> Option<Vec<PluginEffect>> {
        let state = self.tab_state(instance, plugin);
        let (state, effects) = self
            .call_in(plugin, Some(game_dir.clone()), move |plugin, ctx| {
                plugin
                    .instance_tab()
                    .ok_or_else(|| PluginError::Unavailable("plugin has no instance tab".into()))?
                    .update(ctx, state, action)
            })
            .await?;
        self.tab_states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((instance.to_owned(), plugin.to_owned()), state);
        let manifest = self.registry().await.entries.get(plugin)?.manifest.clone();
        Some(
            effects
                .into_iter()
                .filter_map(|effect| match effect {
                    Effect::RevealGameFile { path } => access::resolve(&game_dir, &manifest, &path)
                        .ok()
                        .filter(|full| full.exists())
                        .map(PluginEffect::Reveal),
                    Effect::SaveAs {
                        suggested_name,
                        bytes,
                    } => Some(PluginEffect::SaveAs {
                        suggested_name: suggested_name
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or_default()
                            .to_owned(),
                        bytes,
                    }),
                    Effect::Toast(text) => Some(PluginEffect::Toast(text)),
                })
                .collect(),
        )
    }
}
