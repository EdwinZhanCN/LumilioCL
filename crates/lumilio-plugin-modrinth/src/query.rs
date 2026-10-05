//! Adapted from Modrinth App packages/ui/src/utils/search.ts (newFilters and getEnvironmentFilterGroups), GPL-3.0-only; ADR 0022.
use crate::protocol::{Kind, Sorting};
use lumilio_plugin_api::content::{Pick, ProjectKind, SearchQuery, Stance};
use url::Url;
const API_BASE: &str = "https://api.modrinth.com";
pub(crate) trait Query {
    fn expression(&self) -> String;
    fn has_loaders(&self) -> bool;
    fn has_environment(&self) -> bool;
    fn url(&self) -> String;
}
impl Query for SearchQuery {
    /// Modrinth's filter expression: clauses joined by `AND`.
    fn expression(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        let mut categories_any: Vec<&str> = Vec::new();
        let mut excluded_categories: Vec<&str> = Vec::new();
        for pick in clean(&self.categories) {
            match (pick.stance, pick.any) {
                (Stance::Exclude, _) => excluded_categories.push(&pick.name),
                (Stance::Include, true) => categories_any.push(&pick.name),
                (Stance::Include, false) => {
                    parts.push(format!("categories = {}", literal(&pick.name)));
                }
            }
        }
        if !self.game_versions.is_empty() {
            parts.push(one_of("game_versions", &strs(&self.game_versions)));
        }
        let mut loaders_in: Vec<&str> = Vec::new();
        if self.has_loaders() {
            for pick in clean(&self.loaders) {
                match pick.stance {
                    Stance::Include if pick.any => loaders_in.push(&pick.name),
                    Stance::Include => parts.push(format!("categories = {}", literal(&pick.name))),
                    Stance::Exclude => excluded_categories.push(&pick.name),
                }
            }
        }
        if !loaders_in.is_empty() {
            parts.push(one_of("categories", &loaders_in));
        }
        if !categories_any.is_empty() {
            parts.push(one_of("categories", &categories_any));
        }
        if self.open_source == Some(Stance::Include) {
            parts.push("open_source = true".to_owned());
        }
        if !excluded_categories.is_empty() {
            parts.push(none_of("categories", &excluded_categories));
        }
        if self.open_source == Some(Stance::Exclude) {
            parts.push("open_source NOT IN [true]".to_owned());
        }
        if !self.hidden_projects.is_empty() {
            parts.push(none_of("project_id", &strs(&self.hidden_projects)));
        }
        if !self.excluded_disclosures.is_empty() {
            parts.push(none_of(
                "disclosure_types",
                &strs(&self.excluded_disclosures),
            ));
        }
        if self.has_environment() && (self.client || self.server) {
            for group in environment_groups(self.client, self.server) {
                let conditions: Vec<String> = group
                    .iter()
                    .map(|value| format!("environment = {}", literal(value)))
                    .collect();
                parts.push(if conditions.len() == 1 {
                    conditions.into_iter().next().unwrap_or_default()
                } else {
                    format!("({})", conditions.join(" OR "))
                });
            }
        }
        parts.push(format!(
            "project_types = {}",
            literal(self.kind.protocol_name())
        ));
        if !self.excluded_types.is_empty() {
            parts.push(none_of("all_project_types", &strs(&self.excluded_types)));
        }
        parts.join(" AND ")
    }

    /// Whether this kind has loaders to filter by.
    fn has_loaders(&self) -> bool {
        matches!(
            self.kind,
            ProjectKind::Mod | ProjectKind::Modpack | ProjectKind::Shader
        )
    }

    /// Only mods and modpacks say which side they run on.
    fn has_environment(&self) -> bool {
        matches!(self.kind, ProjectKind::Mod | ProjectKind::Modpack)
    }

    fn url(&self) -> String {
        let mut url = Url::parse(API_BASE).expect("constant address is valid");
        url.set_path("/v3/search");
        let page_size = self.page_size.clamp(1, 100);
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("limit", &page_size.to_string())
                .append_pair("index", self.sort.protocol_name());
            if !self.text.trim().is_empty() {
                pairs.append_pair("query", self.text.trim());
            }
            pairs.append_pair("new_filters", &self.expression());
            if self.page > 0 {
                pairs.append_pair("offset", &self.page.saturating_mul(page_size).to_string());
            }
        }
        url.into()
    }
}

fn clean(picks: &[Pick]) -> impl Iterator<Item = &Pick> {
    picks.iter().filter(|pick| !pick.name.trim().is_empty())
}

fn strs(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

/// A value as the filter language writes it: booleans bare, text in backticks.
fn literal(value: &str) -> String {
    if value == "true" || value == "false" {
        value.to_owned()
    } else {
        format!("`{value}`")
    }
}

fn one_of(field: &str, values: &[&str]) -> String {
    match values {
        [one] => format!("{field} = {}", literal(one)),
        _ => format!("{field} IN [{}]", list(values)),
    }
}

fn none_of(field: &str, values: &[&str]) -> String {
    format!("{field} NOT IN [{}]", list(values))
}

fn list(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| literal(value))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The environment values that satisfy "works on the client" / "works on a
/// server" / both.
fn environment_groups(client: bool, server: bool) -> Vec<Vec<&'static str>> {
    match (client, server) {
        (true, true) => vec![vec![
            "client_only_server_optional",
            "server_only_client_optional",
            "client_and_server",
            "client_or_server",
            "client_or_server_prefers_both",
        ]],
        (true, false) => vec![vec![
            "client_only",
            "client_only_server_optional",
            "client_or_server_prefers_both",
            "client_or_server",
        ]],
        (false, true) => vec![vec![
            "server_only",
            "dedicated_server_only",
            "server_only_client_optional",
            "client_or_server_prefers_both",
            "client_or_server",
        ]],
        (false, false) => Vec::new(),
    }
}
