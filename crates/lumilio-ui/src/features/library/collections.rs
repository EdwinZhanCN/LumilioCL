use std::rc::Rc;

use gpui::{AppContext as _, Context, Window};

use super::library_actions::Library;
use crate::collections::{CollectionPicker, NamePrompt};
use crate::toast::{self, outcome};
use crate::tr;

impl Library {
    /// Asks for a collection's name: a new one, or another for `original`.
    /// Deferred: this may run inside one of the shell's own updates.
    pub fn ask_collection_name(
        &mut self,
        original: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let taken = self.collection_names();
        let this = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let renaming = original.is_some();
            let from = original.clone();
            let form = cx.new(|cx| {
                NamePrompt::new(
                    original,
                    taken,
                    Rc::new(move |name, _, cx| {
                        let from = from.clone();
                        let _ = this.update(cx, |this, cx| this.save_collection(from, name, cx));
                    }),
                    window,
                    cx,
                )
            });
            let (title, confirm) = if renaming {
                (
                    tr!("collection-rename-title"),
                    tr!("collection-rename-confirm"),
                )
            } else {
                (
                    tr!("library-collection-new"),
                    tr!("collection-create-confirm"),
                )
            };
            NamePrompt::open(form, title, confirm, window, cx);
        });
    }

    /// Makes a collection called `name`, or renames `from` to it.
    fn save_collection(&mut self, from: Option<String>, name: String, cx: &mut Context<Self>) {
        let Some(service) = self.service() else {
            return;
        };
        self.change(
            cx,
            async move {
                match from {
                    Some(from) => service
                        .rename_collection(&from, &name)
                        .await
                        .map(|()| tr!("collection-renamed", name = name.as_str())),
                    None => service
                        .create_collection(&name)
                        .await
                        .map(|()| tr!("collection-created", name = name.as_str())),
                }
            },
            |result, cx| {
                let failed = tr!("collection-save-failed").to_owned();
                toast::post(outcome(result, |text| text, failed), cx);
            },
        );
    }

    /// Removes a collection (after the person confirmed); its games stay.
    pub fn delete_collection(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(service) = self.service() else {
            return;
        };
        self.change(
            cx,
            async move { service.delete_collection(&name).await.map(|()| name) },
            |result, cx| {
                let toast = outcome(
                    result,
                    |name| tr!("collection-deleted", name = name.as_str()),
                    tr!("collection-delete-failed").to_owned(),
                );
                toast::post(toast, cx);
            },
        );
    }

    /// Asks which collections a game belongs to, then files it there.
    /// Deferred, like [`Self::ask_collection_name`].
    pub fn ask_collections_of(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.memberships_of(&id);
        let this = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let form = cx.new(|cx| {
                CollectionPicker::new(
                    rows,
                    Rc::new(move |membership, _, cx| {
                        let id = id.clone();
                        let _ = this.update(cx, |this, cx| this.file_game(id, membership, cx));
                    }),
                    window,
                    cx,
                )
            });
            CollectionPicker::open(form, window, cx);
        });
    }

    /// Puts a game in exactly these collections, making the new one first.
    fn file_game(
        &mut self,
        id: String,
        membership: crate::collections::Membership,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service() else {
            return;
        };
        self.change(
            cx,
            async move {
                let mut collections = membership.collections;
                if let Some(new) = membership.new {
                    service.create_collection(&new).await?;
                    collections.push(new);
                }
                service.set_game_collections(&id, &collections).await
            },
            |result, cx| {
                let toast = outcome(
                    result,
                    |()| tr!("collection-updated").to_owned(),
                    tr!("collection-update-failed").to_owned(),
                );
                toast::post(toast, cx);
            },
        );
    }

    /// The names of the collections the page shows.
    pub(super) fn collection_names(&self) -> Vec<String> {
        self.inputs
            .collections
            .iter()
            .map(|collection| collection.name.clone())
            .collect()
    }

    /// Each collection with whether this game is in it, for the picker.
    pub(super) fn memberships_of(&self, id: &str) -> Vec<(String, bool)> {
        self.inputs
            .collections
            .iter()
            .map(|collection| {
                (
                    collection.name.clone(),
                    collection.members.iter().any(|member| member == id),
                )
            })
            .collect()
    }
}
