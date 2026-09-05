//! localcode's model picker inside the TUI: model first, then quant.
//!
//! The catalog is NOT reimplemented here. When `LOCALCODE_CONTROL_URL` is set,
//! the launcher's supervisor (`codex-agent/localcode_supervisor.py`) serves
//! localcode's own catalog modules over localhost — every catalog model
//! (downloaded or not, ★ from `recommend()`), then every quant the HF repo
//! ships with size, fit badge and downloaded marker — and performs the
//! download + server switch. This file only renders those lists with the
//! TUI's selection widget and drives the switch to completion.

use super::*;
use crate::status_indicator_widget::StatusDetailsCapitalization;
use ratatui::text::Span;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

pub(super) const LOCALCODE_MODEL_VIEW_ID: &str = "localcode-model";
pub(super) const LOCALCODE_QUANT_VIEW_ID: &str = "localcode-quant";

/// Base URL of the supervisor's control API, when this TUI runs as
/// localcode's front end.
pub(crate) fn localcode_control_url() -> Option<String> {
    std::env::var("LOCALCODE_CONTROL_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LocalcodeGroup {
    pub key: String,
    pub display_name: String,
    pub maker: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub current: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LocalcodeCatalog {
    #[serde(default)]
    pub ram_gb: u64,
    #[serde(default)]
    pub current: Option<String>,
    pub groups: Vec<LocalcodeGroup>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LocalcodeQuant {
    pub filename: String,
    pub alias: String,
    pub label: String,
    pub size_gb: f64,
    pub fit: String,
    #[serde(default)]
    pub tok_s: Option<u64>,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub downloaded: bool,
    #[serde(default)]
    pub current: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LocalcodeQuants {
    pub group: String,
    pub display_name: String,
    pub maker: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub ram_gb: u64,
    pub quants: Vec<LocalcodeQuant>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LocalcodeStatus {
    pub state: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub pct: Option<u64>,
}

async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("localcode supervisor unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("localcode supervisor returned {}", resp.status()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("bad reply from localcode supervisor: {e}"))
}

async fn post_json(url: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let resp = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(30))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("localcode supervisor unreachable: {e}"))?;
    let status = resp.status();
    let value = resp
        .json::<serde_json::Value>()
        .await
        .unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        let msg = value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("request rejected")
            .to_string();
        return Err(msg);
    }
    Ok(value)
}

fn fit_glyph(fit: &str) -> &'static str {
    match fit {
        "fits" => "✓",
        "tight" => "~",
        "too_big" => "✗",
        _ => "?",
    }
}

fn fit_words(fit: &str) -> &'static str {
    match fit {
        "fits" => "fits",
        "tight" => "tight",
        "too_big" => "too big",
        _ => "",
    }
}

impl ChatWidget {
    /// Level 1: every catalog model. Fetches the catalog, then renders.
    pub(crate) fn open_localcode_model_popup(&mut self, base: String) {
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_json::<LocalcodeCatalog>(&format!("{base}/catalog")).await;
            tx.send(AppEvent::LocalcodeCatalogLoaded { result });
        });
    }

    pub(crate) fn on_localcode_catalog_loaded(&mut self, result: Result<LocalcodeCatalog, String>) {
        let catalog = match result {
            Ok(c) => c,
            Err(e) => {
                self.add_info_message(format!("Could not list models: {e}"), /*hint*/ None);
                return;
            }
        };
        let mut items: Vec<SelectionItem> = Vec::new();
        for g in catalog.groups.iter() {
            let mut bits = vec![g.maker.clone()];
            if !g.license.is_empty() {
                bits.push(g.license.clone());
            }
            let group_key = g.key.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::LocalcodeOpenQuants {
                    group: group_key.clone(),
                });
            })];
            let prefix: Vec<Span<'static>> = if g.recommended {
                vec!["★ ".yellow()]
            } else {
                vec!["  ".into()]
            };
            items.push(SelectionItem {
                name: g.display_name.clone(),
                name_prefix_spans: prefix,
                description: Some(bits.join(" · ")),
                is_current: g.current,
                actions,
                dismiss_on_select: false,
                dismiss_parent_on_child_accept: true,
                search_value: Some(format!("{} {}", g.display_name, g.maker)),
                ..Default::default()
            });
        }
        let subtitle = format!(
            "Model first, then quant · {} GB Mac · ★ recommended for you",
            catalog.ram_gb
        );
        let header = self.model_menu_header("Choose a model", &subtitle);
        self.show_model_selection_view(SelectionViewParams {
            view_id: Some(LOCALCODE_MODEL_VIEW_ID),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            ..Default::default()
        });
    }

    /// Level 2: every quant the repo ships for the chosen model.
    pub(crate) fn open_localcode_quants(&mut self, base: String, group: String) {
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result =
                fetch_json::<LocalcodeQuants>(&format!("{base}/quants?group={group}")).await;
            tx.send(AppEvent::LocalcodeQuantsLoaded { result });
        });
    }

    pub(crate) fn on_localcode_quants_loaded(&mut self, result: Result<LocalcodeQuants, String>) {
        let q = match result {
            Ok(q) => q,
            Err(e) => {
                self.add_info_message(format!("Could not list quants: {e}"), /*hint*/ None);
                return;
            }
        };
        if q.quants.is_empty() {
            self.add_info_message(
                format!(
                    "No quants found for {}. Offline, or this repo ships no GGUF quants yet.",
                    q.display_name
                ),
                /*hint*/ None,
            );
            return;
        }
        let mut items: Vec<SelectionItem> = Vec::new();
        let mut initial: Option<usize> = None;
        for (i, quant) in q.quants.iter().enumerate() {
            let mut bits = vec![format!("{:.1} GB", quant.size_gb)];
            if let Some(t) = quant.tok_s {
                bits.push(format!("~{t} tok/s"));
            }
            if quant.downloaded {
                bits.push("downloaded".to_string());
            } else {
                bits.push(fit_words(&quant.fit).to_string());
            }
            let star: Span<'static> = if quant.recommended {
                "★ ".yellow()
            } else {
                "  ".into()
            };
            let glyph: Span<'static> = match quant.fit.as_str() {
                "fits" => format!("{} ", fit_glyph(&quant.fit)).green(),
                "tight" => format!("{} ", fit_glyph(&quant.fit)).yellow(),
                _ => format!("{} ", fit_glyph(&quant.fit)).red(),
            };
            let (group, filename, alias) =
                (q.group.clone(), quant.filename.clone(), quant.alias.clone());
            let display = format!("{} {}", q.display_name, quant.label);
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::LocalcodeSelectQuant {
                    group: group.clone(),
                    filename: filename.clone(),
                    alias: alias.clone(),
                    display: display.clone(),
                });
            })];
            if quant.current || (initial.is_none() && quant.recommended) {
                initial = Some(i);
            }
            items.push(SelectionItem {
                name: quant.label.clone(),
                name_prefix_spans: vec![star, glyph],
                description: Some(bits.join(" · ")),
                is_current: quant.current,
                actions,
                dismiss_on_select: true,
                search_value: Some(quant.label.clone()),
                ..Default::default()
            });
        }
        let mut title = format!("{} · {}", q.display_name, q.maker);
        if !q.license.is_empty() {
            title.push_str(&format!(" · {}", q.license));
        }
        let subtitle = format!(
            "★ best@{}GB · ✓ fits · ~ tight · ✗ too big · Enter → use (downloads if needed) · Esc ← back",
            q.ram_gb
        );
        let header = self.model_menu_header(&title, &subtitle);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(LOCALCODE_QUANT_VIEW_ID),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            initial_selected_idx: initial,
            ..Default::default()
        });
    }

    /// Ask the supervisor to download (if needed) and switch, then poll its
    /// status into the TUI's status indicator until ready or failed.
    pub(crate) fn localcode_select_quant(
        &mut self,
        base: String,
        group: String,
        filename: String,
        alias: String,
        display: String,
    ) {
        if self.localcode_switch.is_some() {
            self.add_info_message(
                "A model switch is already in progress.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if alias == self.current_model() {
            self.add_info_message(format!("{display} is already the current model."), None);
            return;
        }
        self.localcode_switch = Some(alias.clone());
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane.update_status(
            format!("Switching to {display}"),
            Some("contacting localcode…".to_string()),
            StatusDetailsCapitalization::Preserve,
            1,
        );
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let body = serde_json::json!({ "group": group, "filename": filename });
            if let Err(e) = post_json(&format!("{base}/select"), body).await {
                tx.send(AppEvent::LocalcodeSwitchStatus { result: Err(e) });
                return;
            }
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let result = fetch_json::<LocalcodeStatus>(&format!("{base}/status")).await;
                let done = match &result {
                    Ok(s) => matches!(s.state.as_str(), "ready" | "error" | "idle"),
                    Err(_) => true,
                };
                tx.send(AppEvent::LocalcodeSwitchStatus { result });
                if done {
                    break;
                }
            }
        });
    }

    pub(crate) fn on_localcode_switch_status(&mut self, result: Result<LocalcodeStatus, String>) {
        let Some(alias) = self.localcode_switch.clone() else {
            return;
        };
        let status = match result {
            Ok(s) => s,
            Err(e) => {
                self.localcode_switch = None;
                self.bottom_pane.hide_status_indicator();
                self.add_info_message(format!("Model switch failed: {e}"), /*hint*/ None);
                return;
            }
        };
        let model = status.model.clone().unwrap_or_else(|| alias.clone());
        match status.state.as_str() {
            "downloading" => {
                let detail = match (status.pct, status.detail.unwrap_or_default()) {
                    (_, d) if !d.trim().is_empty() => d,
                    (Some(p), _) => format!("{p}%"),
                    (None, _) => "starting…".to_string(),
                };
                self.bottom_pane.update_status(
                    format!("Downloading {model}"),
                    Some(detail),
                    StatusDetailsCapitalization::Preserve,
                    1,
                );
            }
            "loading" => {
                self.bottom_pane.update_status(
                    format!("Loading {model}"),
                    Some("starting the local server…".to_string()),
                    StatusDetailsCapitalization::Preserve,
                    1,
                );
            }
            "ready" => {
                self.localcode_switch = None;
                self.bottom_pane.hide_status_indicator();
                // Thinking stays off unless the user chose an effort explicitly:
                // persisting `None` (unset) would clear the config key and turn
                // hidden thinking back on for the new model.
                let effort = self
                    .effective_reasoning_effort()
                    .unwrap_or(ReasoningEffortConfig::None);
                self.app_event_tx.send(AppEvent::UpdateModel(model.clone()));
                self.app_event_tx
                    .send(AppEvent::UpdateReasoningEffort(Some(effort.clone())));
                self.app_event_tx.send(AppEvent::PersistModelSelection {
                    model,
                    effort: Some(effort),
                });
            }
            _ => {
                self.localcode_switch = None;
                self.bottom_pane.hide_status_indicator();
                let detail = status.detail.unwrap_or_else(|| "unknown error".to_string());
                self.add_info_message(
                    format!("Model switch to {model} failed: {detail}"),
                    /*hint*/ None,
                );
            }
        }
    }
}
