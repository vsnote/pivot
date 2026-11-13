use crate::components::confirm_dialog::{render_confirm_dialog, ConfirmDialogSpec, ConfirmVariant};
use crate::components::header::render_header;
use crate::components::icons::lucide_icon;
use crate::i18n::{client_type_label, Locale};
use crate::pricing::{
    build_rate_map, compute_today_summary_from_db, ModelRate, TodayUsageSummary,
};
use crate::theme::{
    load_provider_pools, load_ui_preferences, remove_provider_pool, save_provider_pool,
    save_ui_preferences, sync_gpui_component_theme, ThemePalette, ThemePreference, UiPreferences,
};
use crate::tray::{
    self, TrayClientItem, TrayCommand, TrayMenuSnapshot, TrayModelOption, TrayProviderItem,
    TrayProviderModelGroup, TrayUpdateBanner,
};
use crate::updater::{
    self, InstallProgressEvent, UpdateCheckState, UpdateReleaseInfo, RELEASE_LATEST_PAGE_URL,
};
use crate::views::clients::model_select::{
    build_grouped_models, resolve_default_model_for_client, resolve_display_label,
};
use crate::views::clients::{
    render_client_model_popover, render_clients_view, render_conflict_dialog,
    ClientModelSelectState,
};
use crate::views::dashboard::{
    compute_time_window, render_dashboard_view, DashboardLogFilter, DashboardSubView,
    DashboardTimeRange, ProviderMetricMode,
};
use crate::views::providers::list::render_provider_row;
use crate::views::providers::{
    is_subscription_provider, render_provider_context_menu, render_provider_editor,
    render_provider_list, AbandonAction, ProviderContextMenuState, ProviderEditorState,
    PROVIDER_ROW_HEIGHT,
};
use crate::views::settings::{render_settings_view, SettingsBanner};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::presets::Preset;
use pivot_core::db::{DashboardRawStats, Provider, RequestLog, SubscriptionAccount};
use pivot_core::interceptor::InterceptorInfo;
use pivot_core::service;
use pivot_core::state::AppState;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveTab {
    Providers,
    Clients,
    Logs,
    Settings,
}

#[derive(Clone, Copy, Debug)]
pub struct GatewayStatusSnapshot {
    pub is_running: bool,
    pub port: u16,
    pub uptime_secs: u64,
}

#[derive(Clone, Debug)]
pub struct ProviderDragState {
    pub id: String,
    pub initial_index: usize,
    pub start_x: f32,
    pub start_y: f32,
    pub pointer_y: f32,
    pub active: bool,
    pub preview_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum PendingConfirm {
    AbandonEditor(AbandonAction),
    ClearLogs,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubAccountStat {
    pub total: usize,
    pub available: usize,
    pub has_auth_error: bool,
}

pub fn compute_sub_stat(accounts: &[pivot_core::db::SubscriptionAccount]) -> SubAccountStat {
    let now_sec = chrono::Utc::now().timestamp();
    let total = accounts.len();
    let has_auth_error = accounts.iter().any(|a| {
        a.enabled
            && (a.auth_invalid
                || (a.refresh_token.as_deref().unwrap_or("").is_empty()
                    && a.expires_at > 0
                    && a.expires_at <= now_sec))
    });
    let available = accounts
        .iter()
        .filter(|a| {
            a.enabled
                && !a.auth_invalid
                && a.cooldown_until <= now_sec
                && !(a.refresh_token.as_deref().unwrap_or("").is_empty()
                    && a.expires_at > 0
                    && a.expires_at <= now_sec)
        })
        .count();
    SubAccountStat {
        total,
        available,
        has_auth_error,
    }
}

pub struct PivotApp {
    pub core_state: AppState,
    pub tokio_handle: tokio::runtime::Handle,
    pub root_focus: FocusHandle,
    pub main_scroll_handle: ScrollHandle,
    pub trigger_bounds: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,

    pub active_tab: ActiveTab,
    pub prefs: UiPreferences,
    pub initial_loaded: bool,
    pub gateway_status: GatewayStatusSnapshot,

    pub presets: Vec<Preset>,
    pub providers: Vec<Provider>,
    pub saved_pools: HashMap<String, Vec<String>>,
    pub sub_account_stats: HashMap<String, SubAccountStat>,
    pub model_rates: HashMap<String, ModelRate>,
    pub model_display_names: HashMap<String, String>,

    pub hovered_provider_id: Option<String>,
    pub copied_id: Option<String>,
    pub copy_seq: u64,
    pub latencies: HashMap<String, i64>,
    pub testing_latencies: HashMap<String, bool>,
    pub toggling_providers: HashMap<String, bool>,
    pub provider_drag: Option<ProviderDragState>,
    pub context_menu: Option<ProviderContextMenuState>,
    pub delete_provider_target: Option<Provider>,

    pub is_provider_editor_open: bool,
    pub editor: Option<ProviderEditorState>,

    pub interceptors: Vec<InterceptorInfo>,
    pub interceptor_op_seq: u64,
    pub toggling_interceptor_id: Option<String>,
    pub conflict_target: Option<InterceptorInfo>,
    pub client_model_select: Option<ClientModelSelectState>,
    pub universal_card_expanded: bool,

    pub logs: Vec<RequestLog>,
    pub today_summary: TodayUsageSummary,
    pub dashboard_stats: DashboardRawStats,
    pub dashboard_req_seq: u64,
    pub dashboard_sub_view: DashboardSubView,
    pub dashboard_range: DashboardTimeRange,
    pub dashboard_metric: ProviderMetricMode,
    pub dashboard_log_filter: DashboardLogFilter,
    pub expanded_log_id: Option<String>,
    pub hovered_chart_bucket: Option<usize>,

    pub settings_banner: Option<SettingsBanner>,
    pub update_state: UpdateCheckState,
    pub update_version: Option<String>,
    pub update_check_seq: u64,

    pub pending_open_add_provider: bool,
    pub pending_confirm: Option<PendingConfirm>,
    pub confirm_busy: bool,
    pub global_error: Option<String>,
}

fn collect_sub_stats(state: &AppState, providers: &[Provider]) -> HashMap<String, SubAccountStat> {
    let mut map = HashMap::new();
    for p in providers {
        if is_subscription_provider(p) {
            let stat = service::get_subscription_accounts(state, p.id.clone())
                .map(|v| compute_sub_stat(&v))
                .unwrap_or_default();
            map.insert(p.id.clone(), stat);
        }
    }
    map
}

fn resolve_display_names_for_models<'a>(
    names: impl Iterator<Item = &'a String>,
    existing: &mut HashMap<String, String>,
) {
    let missing: Vec<String> = names
        .filter(|m| !m.trim().is_empty() && !existing.contains_key(*m))
        .cloned()
        .collect();
    if missing.is_empty() {
        return;
    }
    let caps = service::get_model_capabilities(missing);
    if let Some(obj) = caps.as_object() {
        for (k, v) in obj {
            if let Some(dn) = v.get("displayName").and_then(|d| d.as_str()) {
                existing.insert(k.clone(), dn.to_string());
            }
        }
    }
}

impl PivotApp {
    pub fn new(
        core_state: AppState,
        tokio_handle: tokio::runtime::Handle,
        log_rx: broadcast::Receiver<RequestLog>,
        tray_rx: mpsc::UnboundedReceiver<TrayCommand>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let prefs = load_ui_preferences();
        let saved_pools = load_provider_pools();
        let presets = service::get_presets();
        let model_rates = build_rate_map();
        let port = core_state.port;
        let root_focus = cx.focus_handle();
        root_focus.focus(window, cx);

        let mut model_display_names = HashMap::new();
        for pr in &presets {
            resolve_display_names_for_models(pr.default_models.iter(), &mut model_display_names);
        }

        let mut app = Self {
            core_state,
            tokio_handle,
            root_focus,
            main_scroll_handle: ScrollHandle::new(),
            trigger_bounds: Rc::new(RefCell::new(HashMap::new())),
            active_tab: ActiveTab::Providers,
            prefs,
            initial_loaded: false,
            gateway_status: GatewayStatusSnapshot {
                is_running: true,
                port,
                uptime_secs: 0,
            },
            presets,
            providers: Vec::new(),
            saved_pools,
            sub_account_stats: HashMap::new(),
            model_rates,
            model_display_names,
            hovered_provider_id: None,
            copied_id: None,
            copy_seq: 0,
            latencies: HashMap::new(),
            testing_latencies: HashMap::new(),
            toggling_providers: HashMap::new(),
            provider_drag: None,
            context_menu: None,
            delete_provider_target: None,
            is_provider_editor_open: false,
            editor: None,
            interceptors: Vec::new(),
            interceptor_op_seq: 0,
            toggling_interceptor_id: None,
            conflict_target: None,
            client_model_select: None,
            universal_card_expanded: true,
            logs: Vec::new(),
            today_summary: TodayUsageSummary::default(),
            dashboard_stats: DashboardRawStats::default(),
            dashboard_req_seq: 0,
            dashboard_sub_view: DashboardSubView::Overview,
            dashboard_range: DashboardTimeRange::All,
            dashboard_metric: ProviderMetricMode::Calls,
            dashboard_log_filter: DashboardLogFilter::All,
            expanded_log_id: None,
            hovered_chart_bucket: None,
            settings_banner: None,
            update_state: UpdateCheckState::Idle,
            update_version: None,
            update_check_seq: 0,
            pending_open_add_provider: false,
            pending_confirm: None,
            confirm_busy: false,
            global_error: None,
        };

        app.sync_tray_snapshot();
        app.refresh_all(cx);
        app.spawn_log_listener(log_rx, cx);
        app.spawn_tray_listener(tray_rx, cx);
        app.spawn_periodic_poller(cx);
        app.check_for_updates_silently(cx);
        app
    }

    pub fn sync_tray_snapshot(&self) {
        let locale = self.prefs.locale;
        let grouped = build_grouped_models(&self.providers);

        let model_groups: Vec<TrayProviderModelGroup> = grouped
            .iter()
            .map(|g| TrayProviderModelGroup {
                provider_name: g.name.clone(),
                models: g
                    .models
                    .iter()
                    .map(|m| TrayModelOption {
                        value: m.value.clone(),
                        raw_id: m.label.clone(),
                        label: m.label.clone(),
                    })
                    .collect(),
            })
            .collect();

        let clients: Vec<TrayClientItem> = self
            .interceptors
            .iter()
            .filter(|c| c.installed)
            .map(|c| {
                let fallback_default = resolve_default_model_for_client(&c.id, &self.providers);
                let effective_model = c
                    .model
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .or(fallback_default)
                    .unwrap_or_default();
                let (display_model, _) = resolve_display_label(&effective_model, &grouped, "");
                TrayClientItem {
                    id: c.id.clone(),
                    name: client_type_label(&c.id, locale),
                    is_intercepted: c.is_intercepted,
                    is_overwritten: c.is_overwritten,
                    active_model_value: effective_model,
                    active_model_display: display_model,
                }
            })
            .collect();

        let providers: Vec<TrayProviderItem> = self
            .providers
            .iter()
            .map(|p| {
                let sub_status = if is_subscription_provider(p) {
                    self.sub_account_stats
                        .get(&p.id)
                        .map(|st| (st.available, st.total, st.has_auth_error))
                } else {
                    None
                };
                TrayProviderItem {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    enabled: p.enabled,
                    sub_status,
                }
            })
            .collect();

        let update_banner = match &self.update_state {
            UpdateCheckState::ReadyToRestart { version, .. } => {
                TrayUpdateBanner::ReadyToRestart(version.clone())
            }
            UpdateCheckState::Available(info)
            | UpdateCheckState::Downloading { info, .. }
            | UpdateCheckState::Installing { info } => {
                TrayUpdateBanner::Available(info.version.clone())
            }
            _ => match &self.update_version {
                Some(ver) if !ver.is_empty() => TrayUpdateBanner::Available(ver.clone()),
                _ => TrayUpdateBanner::None,
            },
        };

        tray::update_tray_snapshot(TrayMenuSnapshot {
            locale,
            is_running: self.gateway_status.is_running,
            port: self.gateway_status.port,
            today: self.today_summary,
            clients,
            model_groups,
            providers,
            update_banner,
        });
    }

    fn spawn_log_listener(
        &self,
        mut log_rx: broadcast::Receiver<RequestLog>,
        cx: &mut Context<Self>,
    ) {
        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| loop {
            let next_res = handle
                .spawn(async move {
                    let res = log_rx.recv().await;
                    (res, log_rx)
                })
                .await;

            let Ok((recv_item, returned_rx)) = next_res else {
                break;
            };
            log_rx = returned_rx;

            match recv_item {
                Ok(new_log) => {
                    let keep_going = this
                        .update(cx, |app, cx| {
                            resolve_display_names_for_models(
                                [&new_log.routed_model, &new_log.incoming_model].into_iter(),
                                &mut app.model_display_names,
                            );
                            let errors_only =
                                app.dashboard_log_filter == DashboardLogFilter::Errors;
                            let is_err = !(200..300).contains(&new_log.status_code);
                            if let Some(existing) =
                                app.logs.iter_mut().find(|l| l.id == new_log.id)
                            {
                                *existing = new_log;
                            } else if !errors_only || is_err {
                                app.logs.insert(0, new_log);
                                if app.logs.len() > 200 {
                                    app.logs.truncate(200);
                                }
                            }
                            app.refresh_dashboard_data(cx);
                            cx.notify();
                        })
                        .is_ok();
                    if !keep_going {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = this.update(cx, |app, cx| {
                        app.refresh_dashboard_data(cx);
                    });
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        })
        .detach();
    }

    fn spawn_tray_listener(
        &self,
        mut tray_rx: mpsc::UnboundedReceiver<TrayCommand>,
        cx: &mut Context<Self>,
    ) {
        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| loop {
            let next_res = handle
                .spawn(async move {
                    let cmd = tray_rx.recv().await;
                    (cmd, tray_rx)
                })
                .await;

            let Ok((Some(cmd), returned_rx)) = next_res else {
                break;
            };
            tray_rx = returned_rx;

            let ok = this
                .update(cx, |app, cx| match cmd {
                    TrayCommand::ShowWindow {
                        tab,
                        open_add_provider,
                    } => {
                        if let Some(target_tab) = tab {
                            app.switch_tab(target_tab, cx);
                        }
                        if open_add_provider {
                            app.pending_open_add_provider = true;
                        }
                        tray::reveal_main_window();
                        cx.activate(true);
                        cx.notify();
                    }
                    TrayCommand::ToggleClient { client_id } => {
                        if let Some(client) =
                            app.interceptors.iter().find(|c| c.id == client_id).cloned()
                        {
                            if client.is_overwritten
                                || (!client.is_intercepted && client.conflict.is_some())
                            {
                                app.switch_tab(ActiveTab::Clients, cx);
                                app.conflict_target = Some(client);
                                tray::reveal_main_window();
                                cx.activate(true);
                                cx.notify();
                            } else {
                                let next_enable = !client.is_intercepted;
                                app.handle_client_switch(client, next_enable, cx);
                            }
                        }
                    }
                    TrayCommand::SelectClientModel { client_id, model } => {
                        app.select_client_model(client_id, model, cx);
                    }
                    TrayCommand::ToggleProvider {
                        provider_id,
                        enabled,
                    } => {
                        app.toggle_provider(provider_id, enabled, cx);
                    }
                    TrayCommand::CopyText(text) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                    TrayCommand::QuitApp => {
                        tray::mark_quitting();
                        cx.quit();
                    }
                })
                .is_ok();
            if !ok {
                break;
            }
        })
        .detach();
    }

    fn spawn_periodic_poller(&self, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| loop {
            let sleep_res = handle
                .spawn(async move {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                })
                .await;
            if sleep_res.is_err() {
                break;
            }

            let Ok((inter_seq, dash_seq)) =
                this.read_with(cx, |app, _| (app.interceptor_op_seq, app.dashboard_req_seq))
            else {
                break;
            };

            let st = state.clone();
            let poll_res = handle
                .spawn(async move {
                    let gw = service::get_gateway_status(&st).await;
                    let inter = service::get_interceptor_status(&st).await;
                    let rates = build_rate_map();
                    let today = compute_today_summary_from_db(&st, &rates);
                    (gw, inter, rates, today)
                })
                .await;

            let Ok((gw_res, inter_res, rates, today)) = poll_res else {
                break;
            };

            let alive = this
                .update(cx, |app, cx| {
                    if let Ok(val) = gw_res {
                        app.gateway_status.is_running = val
                            .get("isRunning")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        app.gateway_status.uptime_secs = val
                            .get("uptimeSecs")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }
                    if app.interceptor_op_seq == inter_seq
                        && app.toggling_interceptor_id.is_none()
                    {
                        if let Ok(interceptors) = inter_res {
                            app.interceptors = interceptors;
                        }
                    }
                    if !rates.is_empty() {
                        app.model_rates = rates;
                    }
                    if app.dashboard_req_seq == dash_seq {
                        app.today_summary = today;
                    }
                    app.sync_tray_snapshot();
                    cx.notify();
                })
                .is_ok();
            if !alive {
                break;
            }
        })
        .detach();
    }

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let range = self.dashboard_range;
        let errors_only = self.dashboard_log_filter == DashboardLogFilter::Errors;
        let inter_seq = self.interceptor_op_seq;
        let dash_seq = self.dashboard_req_seq;

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let p = service::get_providers(&state).await;
                    let i = service::get_interceptor_status(&state).await;
                    let (since, buckets, today_start) = compute_time_window(range);
                    let raw_stats =
                        service::get_dashboard_stats(&state, since, buckets, today_start);
                    let l = service::get_logs_in_window(&state, since, errors_only, 200);
                    let gw = service::get_gateway_status(&state).await;
                    let stats = match &p {
                        Ok(list) => collect_sub_stats(&state, list),
                        Err(_) => HashMap::new(),
                    };
                    let rates = build_rate_map();
                    let today = compute_today_summary_from_db(&state, &rates);
                    (p, i, raw_stats, l, gw, stats, rates, today)
                })
                .await;

            if let Ok((p_res, i_res, raw_stats_res, l_res, gw_res, stats, rates, today)) = res {
                let _ = this.update(cx, |app, cx| {
                    if let Ok(providers) = p_res {
                        for prov in &providers {
                            resolve_display_names_for_models(
                                prov.models.iter(),
                                &mut app.model_display_names,
                            );
                        }
                        app.providers = providers;
                    }
                    app.sub_account_stats = stats;
                    if app.interceptor_op_seq == inter_seq
                        && app.toggling_interceptor_id.is_none()
                    {
                        if let Ok(interceptors) = i_res {
                            app.interceptors = interceptors;
                        }
                    }
                    if app.dashboard_req_seq == dash_seq {
                        if let Ok(st) = raw_stats_res {
                            resolve_display_names_for_models(
                                st.route_model_slices.iter().map(|s| &s.model),
                                &mut app.model_display_names,
                            );
                            app.dashboard_stats = st;
                        }
                        if let Ok(logs) = l_res {
                            resolve_display_names_for_models(
                                logs.iter().flat_map(|l| [&l.routed_model, &l.incoming_model]),
                                &mut app.model_display_names,
                            );
                            app.logs = logs;
                        }
                        app.today_summary = today;
                    }
                    if let Ok(val) = gw_res {
                        app.gateway_status.is_running = val
                            .get("isRunning")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        app.gateway_status.uptime_secs = val
                            .get("uptimeSecs")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }
                    app.model_rates = rates;
                    app.initial_loaded = true;
                    app.sync_tray_snapshot();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn refresh_dashboard_data(&mut self, cx: &mut Context<Self>) {
        self.dashboard_req_seq = self.dashboard_req_seq.wrapping_add(1);
        let seq = self.dashboard_req_seq;
        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let range = self.dashboard_range;
        let errors_only = self.dashboard_log_filter == DashboardLogFilter::Errors;
        let rates = self.model_rates.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let (since, buckets, today_start) = compute_time_window(range);
                    let raw_stats =
                        service::get_dashboard_stats(&state, since, buckets, today_start);
                    let logs = service::get_logs_in_window(&state, since, errors_only, 200);
                    let today = compute_today_summary_from_db(&state, &rates);
                    (raw_stats, logs, today)
                })
                .await;

            if let Ok((stats_res, logs_res, today)) = res {
                let _ = this.update(cx, |app, cx| {
                    if app.dashboard_req_seq != seq {
                        return;
                    }
                    if let Ok(st) = stats_res {
                        resolve_display_names_for_models(
                            st.route_model_slices.iter().map(|s| &s.model),
                            &mut app.model_display_names,
                        );
                        app.dashboard_stats = st;
                    }
                    if let Ok(logs) = logs_res {
                        resolve_display_names_for_models(
                            logs.iter().flat_map(|l| [&l.routed_model, &l.incoming_model]),
                            &mut app.model_display_names,
                        );
                        app.logs = logs;
                    }
                    app.today_summary = today;
                    app.sync_tray_snapshot();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn jump_to_error_logs(&mut self, cx: &mut Context<Self>) {
        self.dashboard_sub_view = DashboardSubView::Logs;
        self.dashboard_log_filter = DashboardLogFilter::Errors;
        self.expanded_log_id = None;
        self.main_scroll_handle.set_offset(point(px(0.0), px(0.0)));
        self.refresh_dashboard_data(cx);
        cx.notify();
    }

    pub fn refresh_providers_only(&mut self, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let p = service::get_providers(&state).await;
                    let stats = match &p {
                        Ok(list) => collect_sub_stats(&state, list),
                        Err(_) => HashMap::new(),
                    };
                    let inter = service::get_interceptor_status(&state).await;
                    (p, stats, inter)
                })
                .await;

            if let Ok((Ok(providers), stats, inter_res)) = res {
                let _ = this.update(cx, |app, cx| {
                    for prov in &providers {
                        resolve_display_names_for_models(
                            prov.models.iter(),
                            &mut app.model_display_names,
                        );
                    }
                    app.providers = providers;
                    app.sub_account_stats = stats;
                    if let Ok(interceptors) = inter_res {
                        app.interceptors = interceptors;
                    }
                    app.sync_tray_snapshot();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    // ================= 顶栏导航与编辑器进出 =================

    pub fn switch_tab(&mut self, tab: ActiveTab, cx: &mut Context<Self>) {
        if self.active_tab == tab && !self.is_provider_editor_open {
            return;
        }
        if self.active_tab == ActiveTab::Providers && self.is_provider_editor_open {
            if let Some(ref ed) = self.editor {
                if ed.has_unsaved_changes(cx) {
                    self.pending_confirm =
                        Some(PendingConfirm::AbandonEditor(AbandonAction::SwitchTab(tab)));
                    cx.notify();
                    return;
                }
            }
            self.is_provider_editor_open = false;
            self.editor = None;
        }

        if self.active_tab == ActiveTab::Providers && tab != ActiveTab::Providers {
            self.latencies.clear();
        }
        if self.active_tab == ActiveTab::Settings
            && tab != ActiveTab::Settings
            && matches!(self.update_state, UpdateCheckState::UpToDate)
        {
            self.update_check_seq = self.update_check_seq.wrapping_add(1);
            self.update_state = UpdateCheckState::Idle;
        }
        self.context_menu = None;
        self.client_model_select = None;
        self.active_tab = tab;
        if tab == ActiveTab::Logs {
            self.hovered_chart_bucket = None;
            self.refresh_dashboard_data(cx);
        }
        self.main_scroll_handle.set_offset(point(px(0.0), px(0.0)));
        cx.notify();
    }

    pub fn request_close_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(ref ed) = self.editor {
            if ed.has_unsaved_changes(cx) {
                self.pending_confirm = Some(PendingConfirm::AbandonEditor(AbandonAction::Leave));
                cx.notify();
                return;
            }
        }
        self.close_editor_immediately(cx);
    }

    pub fn close_editor_immediately(&mut self, cx: &mut Context<Self>) {
        self.is_provider_editor_open = false;
        self.editor = None;
        self.pending_confirm = None;
        cx.notify();
    }

    pub fn open_editor(
        &mut self,
        provider: Option<Provider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_tab = ActiveTab::Providers;
        self.context_menu = None;
        self.hovered_provider_id = None;
        self.saved_pools = load_provider_pools();

        let sub_accounts = match &provider {
            Some(p) if is_subscription_provider(p) => {
                service::get_subscription_accounts(&self.core_state, p.id.clone())
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let precheck_sub = provider
            .as_ref()
            .filter(|p| is_subscription_provider(p) && !sub_accounts.is_empty())
            .map(|p| (p.id.clone(), p.base_url.clone()));

        let ed = ProviderEditorState::new(
            provider,
            &self.presets,
            &self.saved_pools,
            sub_accounts,
            self.prefs.locale,
            window,
            cx,
        );
        self.editor = Some(ed);
        self.is_provider_editor_open = true;
        cx.notify();

        // 静默预加载并检查订阅账号健康状态（若 Token 已过期会自动触发刷新或标记冷却）
        if let Some((pid, base_url)) = precheck_sub {
            let state = self.core_state.clone();
            let handle = self.tokio_handle.clone();
            let pid_clone = pid.clone();
            cx.spawn(async move |this, cx| {
                let res = handle
                    .spawn(async move {
                        let quotas = service::get_all_subscription_quotas(
                            &state,
                            pid_clone.clone(),
                            if base_url.is_empty() {
                                None
                            } else {
                                Some(base_url)
                            },
                        )
                        .await;
                        let accounts = service::get_subscription_accounts(&state, pid_clone)
                            .unwrap_or_default();
                        (quotas, accounts)
                    })
                    .await;
                if let Ok((quotas_res, accounts)) = res {
                    let _ = this.update(cx, |app, cx| {
                        app.sub_account_stats
                            .insert(pid.clone(), compute_sub_stat(&accounts));
                        if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == pid) {
                            ed.sub_accounts = accounts;
                            if let Ok(list) = quotas_res {
                                for q in list {
                                    ed.sub_quotas.insert(q.account_id.clone(), q);
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    // ================= 剪贴板与服务商列表操作 =================

    pub fn copy_to_clipboard(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copy_seq = self.copy_seq.wrapping_add(1);
        let seq = self.copy_seq;
        self.copied_id = Some(id);
        cx.notify();

        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| {
            let _ = handle
                .spawn(async move {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.copy_seq == seq {
                    app.copied_id = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn toggle_provider(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        if self.toggling_providers.contains_key(&id) {
            return;
        }
        self.toggling_providers.insert(id.clone(), true);
        if let Some(p) = self.providers.iter_mut().find(|p| p.id == id) {
            p.enabled = enabled;
        }
        self.sync_tray_snapshot();
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let target_id = id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move { service::set_provider_enabled(&state, target_id, enabled).await })
                .await;

            let _ = this.update(cx, |app, cx| {
                app.toggling_providers.remove(&id);
                if let Ok(Err(err)) = res {
                    app.global_error = Some(err);
                }
                app.refresh_providers_only(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn test_provider_latency(&mut self, id: String, cx: &mut Context<Self>) {
        if self.testing_latencies.contains_key(&id) {
            return;
        }
        self.testing_latencies.insert(id.clone(), true);
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let target_id = id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let lat = service::test_provider_latency(&state, target_id.clone()).await;
                    let sub_stat = service::get_subscription_accounts(&state, target_id)
                        .ok()
                        .map(|v| compute_sub_stat(&v));
                    (lat, sub_stat)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                app.testing_latencies.remove(&id);
                match res {
                    Ok((Ok(ms), sub_stat)) => {
                        if let Some(st) = sub_stat {
                            if app.sub_account_stats.contains_key(&id) {
                                app.sub_account_stats.insert(id.clone(), st);
                            }
                        }
                        app.latencies.insert(id, ms);
                    }
                    Ok((Err(_), sub_stat)) => {
                        if let Some(st) = sub_stat {
                            if app.sub_account_stats.contains_key(&id) {
                                app.sub_account_stats.insert(id.clone(), st);
                            }
                        }
                        app.latencies.insert(id, -1);
                    }
                    _ => {
                        app.latencies.insert(id, -1);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn begin_provider_drag(
        &mut self,
        id: String,
        row_index: usize,
        x: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) {
        if self.providers.len() <= 1 {
            return;
        }
        let preview_ids = self.providers.iter().map(|p| p.id.clone()).collect();
        self.context_menu = None;
        self.hovered_provider_id = None;
        self.provider_drag = Some(ProviderDragState {
            id,
            initial_index: row_index,
            start_x: x,
            start_y: y,
            pointer_y: y,
            active: false,
            preview_ids,
        });
        cx.notify();
    }

    fn handle_root_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.provider_drag.as_mut() else {
            return;
        };
        let cur_x = f32::from(event.position.x);
        let cur_y = f32::from(event.position.y);

        if !drag.active {
            let dx = (cur_x - drag.start_x).abs();
            let dy = (cur_y - drag.start_y).abs();
            if dx < 4.0 && dy < 4.0 {
                return;
            }
            drag.active = true;
        }

        drag.pointer_y = cur_y;
        let len = self.providers.len();
        if len <= 1 {
            return;
        }

        let delta_rows = ((cur_y - drag.start_y) / PROVIDER_ROW_HEIGHT).round() as isize;
        let target_index = (drag.initial_index as isize + delta_rows)
            .clamp(0, (len as isize) - 1) as usize;

        let mut ids: Vec<String> = self.providers.iter().map(|p| p.id.clone()).collect();
        if let Some(orig_pos) = ids.iter().position(|pid| pid == &drag.id) {
            let moved = ids.remove(orig_pos);
            let insert_at = target_index.min(ids.len());
            ids.insert(insert_at, moved);
            drag.preview_ids = ids;
        }
        cx.notify();
    }

    fn handle_root_mouse_up(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.provider_drag.take() else {
            return;
        };
        if !drag.active {
            cx.notify();
            return;
        }

        let current_ids: Vec<String> = self.providers.iter().map(|p| p.id.clone()).collect();
        if drag.preview_ids == current_ids {
            cx.notify();
            return;
        }

        // 乐观更新本地顺序
        let reordered: Vec<Provider> = drag
            .preview_ids
            .iter()
            .filter_map(|id| self.providers.iter().find(|p| &p.id == id).cloned())
            .collect();
        if reordered.len() == self.providers.len() {
            self.providers = reordered;
        }
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let ids_to_save = drag.preview_ids;

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move { service::reorder_providers(&state, ids_to_save).await })
                .await;

            let _ = this.update(cx, |app, cx| {
                if let Ok(Ok(updated)) = res {
                    app.providers = updated;
                    app.sync_tray_snapshot();
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ================= 右键菜单与配额速查 =================

    pub fn open_provider_context_menu(
        &mut self,
        provider: Provider,
        x: f32,
        y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = self.prefs.locale;
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(locale.pick("搜索模型…", "Search models…"))
        });
        cx.subscribe_in(
            &search_input,
            window,
            |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        )
        .detach();

        let is_sub = is_subscription_provider(&provider);
        let accounts = if is_sub {
            service::get_subscription_accounts(&self.core_state, provider.id.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let selected_account_id = accounts
            .iter()
            .find(|a| a.enabled)
            .or_else(|| accounts.first())
            .map(|a| a.id.clone());

        let prov_id = provider.id.clone();
        let base_url = provider.base_url.clone();
        self.hovered_provider_id = None;

        self.context_menu = Some(ProviderContextMenuState {
            provider_id: prov_id.clone(),
            x,
            y,
            search_input,
            scroll_handle: ScrollHandle::new(),
            accounts,
            quotas: HashMap::new(),
            selected_account_id,
            loading_quota: is_sub,
            hovered_model: None,
        });
        cx.notify();

        if is_sub {
            self.refresh_context_menu_quotas(prov_id, base_url, cx);
        }
    }

    pub fn refresh_context_menu_quotas(
        &mut self,
        provider_id: String,
        base_url: String,
        cx: &mut Context<Self>,
    ) {
        let account_ids: Vec<String> = self
            .context_menu
            .as_ref()
            .filter(|m| m.provider_id == provider_id)
            .map(|m| m.accounts.iter().map(|a| a.id.clone()).collect())
            .unwrap_or_default();

        if account_ids.is_empty() {
            if let Some(m) = self.context_menu.as_mut() {
                m.loading_quota = false;
            }
            cx.notify();
            return;
        }

        if let Some(m) = self.context_menu.as_mut() {
            m.loading_quota = true;
        }
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let pid = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let mut out = HashMap::new();
                    for aid in account_ids {
                        if let Ok(q) = service::get_subscription_account_quota(
                            &state,
                            pid.clone(),
                            aid.clone(),
                            Some(base_url.clone()),
                        )
                        .await
                        {
                            out.insert(aid, q);
                        }
                    }
                    out
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                if let Some(menu) = app
                    .context_menu
                    .as_mut()
                    .filter(|m| m.provider_id == provider_id)
                {
                    menu.loading_quota = false;
                    if let Ok(quotas) = res {
                        menu.quotas = quotas;
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // ================= ProviderEditor 交互方法群 =================

    pub fn editor_choose_preset(
        &mut self,
        preset: Option<Preset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 若选择官方订阅预设且本地已存在该订阅服务商实例，直接复用编辑
        if let Some(ref pr) = preset {
            if crate::views::providers::editor::is_subscription_preset(pr) {
                if let Some(existing) = self
                    .providers
                    .iter()
                    .find(|p| {
                        p.preset_id.as_deref() == Some(pr.id.as_str())
                            || (pr.id == "chatgpt_subscription" && p.protocol == "chatgpt")
                    })
                    .cloned()
                {
                    self.open_editor(Some(existing), window, cx);
                    return;
                }
            }
            resolve_display_names_for_models(
                pr.default_models.iter(),
                &mut self.model_display_names,
            );
        }

        let pools = self.saved_pools.clone();
        if let Some(mut ed) = self.editor.take() {
            ed.apply_preset(preset, &pools, window, cx);
            self.editor = Some(ed);
            cx.notify();
        }
    }

    pub fn editor_handle_back(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref ed) = self.editor else {
            return;
        };
        if ed.is_new && !ed.choosing_preset {
            if ed.has_unsaved_changes(cx) {
                self.pending_confirm =
                    Some(PendingConfirm::AbandonEditor(AbandonAction::BackToPresets));
                cx.notify();
                return;
            }
            if let Some(ed_mut) = self.editor.as_mut() {
                ed_mut.choosing_preset = true;
                ed_mut.error_msg = None;
            }
            cx.notify();
        } else {
            self.request_close_editor(cx);
        }
    }

    pub fn editor_change_protocol(
        &mut self,
        next_protocol: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut ed) = self.editor.take() else {
            return;
        };
        if ed.protocol != next_protocol {
            ed.protocol = next_protocol.clone();
            ed.protocol_picker_open = false;
            ed.selected_models.clear();
            ed.model_pool.clear();
            ed.upstream_models = None;
            ed.endpoint_latency = None;
            ed.endpoint_error = None;
            ed.auth_type = if next_protocol == "chatgpt" {
                Some("subscription_pool".to_string())
            } else {
                None
            };
            let default_url = match next_protocol.as_str() {
                "anthropic" => "https://api.anthropic.com/v1",
                "gemini" => "https://generativelanguage.googleapis.com/v1beta",
                "openrouter" => "https://openrouter.ai/api/v1",
                "ollama" => "http://127.0.0.1:11434/v1",
                "chatgpt" => "https://chatgpt.com/backend-api/codex",
                _ => "https://api.openai.com/v1",
            };
            ed.base_url_input
                .update(cx, |s, cx| s.set_value(default_url, window, cx));
        } else {
            ed.protocol_picker_open = false;
        }
        self.editor = Some(ed);
        cx.notify();
    }

    pub fn editor_toggle_keys_visibility(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut ed) = self.editor.take() else {
            return;
        };
        ed.show_keys = !ed.show_keys;
        let masked = !ed.show_keys;
        for ki in &ed.key_inputs {
            ki.update(cx, |s, cx| s.set_masked(masked, window, cx));
        }
        self.editor = Some(ed);
        cx.notify();
    }

    pub fn editor_add_key_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut ed) = self.editor.take() else {
            return;
        };
        let masked = !ed.show_keys;
        let ki = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("sk-...")
                .masked(masked)
        });
        cx.subscribe_in(&ki, window, |_this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        ed.key_inputs.push(ki);
        self.editor = Some(ed);
        cx.notify();
    }

    pub fn editor_remove_key_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(ed) = self.editor.as_mut() {
            if ed.key_inputs.len() > 1 && index < ed.key_inputs.len() {
                ed.key_inputs.remove(index);
                cx.notify();
            }
        }
    }

    pub fn editor_test_endpoint(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.testing_endpoint {
            return;
        }
        let base_url = ed.base_url_input.read(cx).value().trim().to_string();
        if let Some(hint_err) =
            crate::views::providers::editor::validate_base_url_hint(&base_url, self.prefs.locale)
        {
            ed.endpoint_error = Some(hint_err);
            ed.endpoint_latency = None;
            cx.notify();
            return;
        }
        let protocol = ed.protocol.clone();
        let first_key = ed.collect_api_keys(cx).into_iter().next();
        ed.testing_endpoint = true;
        ed.endpoint_error = None;
        ed.endpoint_latency = None;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    service::test_endpoint_latency(&state, protocol, base_url, first_key).await
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                if let Some(ed) = app.editor.as_mut() {
                    ed.testing_endpoint = false;
                    match res {
                        Ok(Ok(ms)) => {
                            ed.endpoint_latency = Some(ms);
                            ed.endpoint_error = None;
                        }
                        Ok(Err(err)) => {
                            ed.endpoint_latency = None;
                            ed.endpoint_error = Some(err);
                        }
                        Err(e) => {
                            ed.endpoint_latency = None;
                            ed.endpoint_error = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn editor_fetch_models(&mut self, cx: &mut Context<Self>) {
        self.editor_fetch_models_with_mode(false, cx);
    }

    fn editor_fetch_models_with_mode(&mut self, auto_select_new: bool, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.fetching_models {
            return;
        }

        let is_sub = ed.is_subscription_mode();
        let provider_id = ed.provider_id.clone();
        let protocol = ed.protocol.clone();
        let base_url = ed.base_url_input.read(cx).value().trim().to_string();
        let api_key = ed.collect_api_keys(cx).into_iter().next().unwrap_or_default();

        if !is_sub && base_url.is_empty() {
            ed.error_msg = Some(
                self.prefs
                    .locale
                    .pick("请先填写接口地址", "Please enter Base URL first")
                    .to_string(),
            );
            cx.notify();
            return;
        }

        ed.fetching_models = true;
        ed.error_msg = None;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let pid_check = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    if is_sub {
                        let sync_res =
                            service::sync_subscription_models(&state, provider_id.clone(), base_url)
                                .await;
                        let accounts =
                            service::get_subscription_accounts(&state, provider_id).ok();
                        (sync_res, accounts)
                    } else {
                        let fetch_res =
                            service::fetch_provider_models(protocol, base_url, api_key).await;
                        (fetch_res, None)
                    }
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                let mut should_auto_save = false;
                let mut updated_stat: Option<SubAccountStat> = None;
                if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == pid_check) {
                    ed.fetching_models = false;
                    match res {
                        Ok((fetch_res, maybe_accounts)) => {
                            if let Some(accs) = maybe_accounts {
                                updated_stat = Some(compute_sub_stat(&accs));
                                ed.sub_accounts = accs;
                            }
                            match fetch_res {
                                Ok(fetched) => {
                                    resolve_display_names_for_models(
                                        fetched.iter(),
                                        &mut app.model_display_names,
                                    );
                                    ed.upstream_models = Some(fetched.clone());
                                    // 合并入候选模型池并持久化，同时记录本次新增的池内模型
                                    let mut seen: HashSet<String> =
                                        ed.model_pool.iter().cloned().collect();
                                    let mut newly_added: Vec<String> = Vec::new();
                                    for m in &fetched {
                                        let t = m.trim();
                                        if !t.is_empty() && seen.insert(t.to_string()) {
                                            let owned = t.to_string();
                                            ed.model_pool.push(owned.clone());
                                            newly_added.push(owned);
                                        }
                                    }
                                    save_provider_pool(&ed.provider_id, &ed.model_pool);

                                    // 若订阅模式当前尚未选任何模型，自动默认选中全池；
                                    // 若是由新增/导入更高阶订阅账号触发的同步，自动将新解锁模型并入已选列表
                                    if is_sub && !fetched.is_empty() {
                                        if ed.selected_models.is_empty() {
                                            ed.selected_models = fetched;
                                            should_auto_save = true;
                                        } else if auto_select_new && !newly_added.is_empty() {
                                            let mut selected_set: HashSet<String> =
                                                ed.selected_models.iter().cloned().collect();
                                            for m in newly_added {
                                                if selected_set.insert(m.clone()) {
                                                    ed.selected_models.push(m);
                                                }
                                            }
                                            should_auto_save = true;
                                        }
                                    }
                                }
                                Err(err) => {
                                    ed.error_msg = Some(err);
                                }
                            }
                        }
                        Err(e) => {
                            ed.error_msg = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }

                if let Some(stat) = updated_stat {
                    app.sub_account_stats.insert(pid_check, stat);
                }
                if should_auto_save {
                    app.editor_auto_save_subscription(cx);
                }
            });
        })
        .detach();
    }

    pub fn editor_toggle_single_model(&mut self, model: String, cx: &mut Context<Self>) {
        if let Some(ed) = self.editor.as_mut() {
            if let Some(pos) = ed.selected_models.iter().position(|m| m == &model) {
                ed.selected_models.remove(pos);
            } else {
                ed.selected_models.push(model);
            }
            ed.auto_saved = false;
            ed.error_msg = None;
            cx.notify();
        }
    }

    pub fn editor_set_models_selected(
        &mut self,
        targets: Vec<String>,
        select: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(ed) = self.editor.as_mut() {
            let target_set: HashSet<&str> = targets.iter().map(|s| s.as_str()).collect();
            if select {
                let existing: HashSet<String> = ed.selected_models.iter().cloned().collect();
                for t in targets {
                    if !existing.contains(&t) {
                        ed.selected_models.push(t);
                    }
                }
            } else {
                ed.selected_models
                    .retain(|m| !target_set.contains(m.as_str()));
            }
            ed.auto_saved = false;
            ed.error_msg = None;
            cx.notify();
        }
    }

    pub fn editor_remove_stale_models(&mut self, cx: &mut Context<Self>) {
        if let Some(ed) = self.editor.as_mut() {
            if let Some(ref up) = ed.upstream_models {
                let up_set: HashSet<&str> = up.iter().map(|s| s.as_str()).collect();
                ed.selected_models.retain(|m| up_set.contains(m.as_str()));
                ed.auto_saved = false;
                cx.notify();
            }
        }
    }

    pub fn editor_delete_pool_model(&mut self, model: String, cx: &mut Context<Self>) {
        if let Some(ed) = self.editor.as_mut() {
            ed.model_pool.retain(|m| m != &model);
            ed.selected_models.retain(|m| m != &model);
            save_provider_pool(&ed.provider_id, &ed.model_pool);
            ed.auto_saved = false;
            cx.notify();
        }
    }

    pub fn editor_add_custom_model(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut ed) = self.editor.take() else {
            return;
        };
        let raw = ed.custom_model_input.read(cx).value().to_string();
        let parts: Vec<String> = raw
            .split([',', '\n', ' '])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !parts.is_empty() {
            resolve_display_names_for_models(parts.iter(), &mut self.model_display_names);
            for m in parts {
                if !ed.model_pool.contains(&m) {
                    ed.model_pool.push(m.clone());
                }
                if !ed.selected_models.contains(&m) {
                    ed.selected_models.push(m);
                }
            }
            save_provider_pool(&ed.provider_id, &ed.model_pool);
            ed.custom_model_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            ed.error_msg = None;
            ed.auto_saved = false;
        }
        self.editor = Some(ed);
        cx.notify();
    }

    // ================= 官方订阅账号池操作 =================

    pub fn editor_start_subscription_oauth(
        &mut self,
        account_type: String,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.connecting_oauth {
            return;
        }
        let provider_id = ed.provider_id.clone();
        ed.connecting_oauth = true;
        ed.error_msg = None;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let pid_clone = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let oauth_res =
                        service::start_subscription_oauth(&state, pid_clone.clone(), account_type)
                            .await;
                    let accounts = service::get_subscription_accounts(&state, pid_clone)
                        .unwrap_or_default();
                    (oauth_res, accounts)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                let mut trigger_sync = false;
                let mut updated_stat: Option<SubAccountStat> = None;
                if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == provider_id) {
                    ed.connecting_oauth = false;
                    match res {
                        Ok((Ok(_acc), accounts)) => {
                            updated_stat = Some(compute_sub_stat(&accounts));
                            ed.sub_accounts = accounts;
                            trigger_sync = true;
                        }
                        Ok((Err(err), accounts)) => {
                            updated_stat = Some(compute_sub_stat(&accounts));
                            ed.sub_accounts = accounts;
                            ed.error_msg = Some(err);
                        }
                        Err(e) => {
                            ed.error_msg = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }
                if let Some(stat) = updated_stat {
                    app.sub_account_stats.insert(provider_id.clone(), stat);
                }
                if trigger_sync {
                    app.editor_fetch_models_with_mode(true, cx);
                    app.editor_auto_save_subscription(cx);
                }
            });
        })
        .detach();
    }

    pub fn editor_import_codex_account(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.importing_codex {
            return;
        }
        let provider_id = ed.provider_id.clone();
        ed.importing_codex = true;
        ed.error_msg = None;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let pid_clone = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let imp_res =
                        service::import_codex_subscription_account(&state, pid_clone.clone()).await;
                    let accounts = service::get_subscription_accounts(&state, pid_clone)
                        .unwrap_or_default();
                    (imp_res, accounts)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                let mut trigger_sync = false;
                let mut updated_stat: Option<SubAccountStat> = None;
                if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == provider_id) {
                    ed.importing_codex = false;
                    match res {
                        Ok((Ok(_), accounts)) => {
                            updated_stat = Some(compute_sub_stat(&accounts));
                            ed.sub_accounts = accounts;
                            trigger_sync = true;
                        }
                        Ok((Err(err), _)) => {
                            ed.error_msg = Some(err);
                        }
                        Err(e) => {
                            ed.error_msg = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }
                if let Some(stat) = updated_stat {
                    app.sub_account_stats.insert(provider_id.clone(), stat);
                }
                if trigger_sync {
                    app.editor_fetch_models_with_mode(true, cx);
                    app.editor_auto_save_subscription(cx);
                }
            });
        })
        .detach();
    }

    fn editor_auto_save_subscription(&mut self, cx: &mut Context<Self>) {
        let payload_opt = self
            .editor
            .as_ref()
            .and_then(|ed| ed.build_provider_payload(self.prefs.locale, cx).ok());
        let Some(provider) = payload_opt else {
            return;
        };

        if let Some(ed) = self.editor.as_mut() {
            save_provider_pool(&ed.provider_id, &ed.model_pool);
            ed.sync_baseline(cx);
            ed.auto_saved = true;
            ed.is_new = false;
            ed.editing_provider = Some(provider.clone());
        }

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| {
            let _ = handle
                .spawn(async move { service::save_provider(&state, provider).await })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.refresh_providers_only(cx);
            });
        })
        .detach();
    }

    pub fn editor_toggle_subscription_account(
        &mut self,
        account_id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        let pid = ed.provider_id.clone();
        let _ = service::toggle_subscription_account(
            &self.core_state,
            pid.clone(),
            account_id,
            enabled,
        );
        ed.sub_accounts =
            service::get_subscription_accounts(&self.core_state, pid.clone()).unwrap_or_default();
        let stat = compute_sub_stat(&ed.sub_accounts);
        self.sub_account_stats.insert(pid, stat);
        cx.notify();
    }

    pub fn editor_delete_subscription_account(
        &mut self,
        account_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        let pid = ed.provider_id.clone();
        let _ = service::delete_subscription_account(&self.core_state, account_id.clone());
        ed.sub_accounts =
            service::get_subscription_accounts(&self.core_state, pid.clone()).unwrap_or_default();
        ed.sub_quotas.remove(&account_id);
        if ed.expanded_quota_account_id.as_deref() == Some(account_id.as_str()) {
            ed.expanded_quota_account_id = None;
        }
        let stat = compute_sub_stat(&ed.sub_accounts);
        self.sub_account_stats.insert(pid, stat);
        cx.notify();
    }

    pub fn editor_toggle_account_quota(&mut self, account_id: String, cx: &mut Context<Self>) {
        let mut need_fetch = false;
        if let Some(ed) = self.editor.as_mut() {
            if ed.expanded_quota_account_id.as_deref() == Some(account_id.as_str()) {
                ed.expanded_quota_account_id = None;
            } else {
                ed.expanded_quota_account_id = Some(account_id.clone());
                if !ed.sub_quotas.contains_key(&account_id) {
                    need_fetch = true;
                }
            }
            cx.notify();
        }
        if need_fetch {
            self.editor_refresh_account_quota(account_id, cx);
        }
    }

    pub fn editor_refresh_account_quota(&mut self, account_id: String, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.loading_quotas.get(&account_id).copied().unwrap_or(false) {
            return;
        }
        let provider_id = ed.provider_id.clone();
        let base_url = ed.base_url_input.read(cx).value().trim().to_string();
        ed.loading_quotas.insert(account_id.clone(), true);
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let aid = account_id.clone();
        let pid = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let quota_res = service::get_subscription_account_quota(
                        &state,
                        pid.clone(),
                        aid,
                        Some(base_url),
                    )
                    .await;
                    let accounts = service::get_subscription_accounts(&state, pid).ok();
                    (quota_res, accounts)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                let mut updated_stat: Option<SubAccountStat> = None;
                if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == provider_id) {
                    ed.loading_quotas.remove(&account_id);
                    if let Ok((quota_res, maybe_accounts)) = res {
                        if let Some(accs) = maybe_accounts {
                            updated_stat = Some(compute_sub_stat(&accs));
                            ed.sub_accounts = accs;
                        }
                        if let Ok(quota) = quota_res {
                            ed.sub_quotas.insert(account_id, quota);
                        }
                    }
                    cx.notify();
                }
                if let Some(stat) = updated_stat {
                    app.sub_account_stats.insert(provider_id, stat);
                }
            });
        })
        .detach();
    }

    pub fn editor_consume_reset_credit(
        &mut self,
        account_id: String,
        credit_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.consuming_credit_id.is_some() {
            return;
        }
        let provider_id = ed.provider_id.clone();
        ed.consuming_credit_id = Some(credit_id);
        ed.error_msg = None;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let aid = account_id.clone();
        let pid = provider_id.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    let credit_res =
                        service::consume_subscription_reset_credit(&state, pid.clone(), aid).await;
                    let accounts = service::get_subscription_accounts(&state, pid).ok();
                    (credit_res, accounts)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                let mut updated_stat: Option<SubAccountStat> = None;
                if let Some(ed) = app.editor.as_mut().filter(|e| e.provider_id == provider_id) {
                    ed.consuming_credit_id = None;
                    match res {
                        Ok((credit_res, maybe_accounts)) => {
                            if let Some(accs) = maybe_accounts {
                                updated_stat = Some(compute_sub_stat(&accs));
                                ed.sub_accounts = accs;
                            }
                            match credit_res {
                                Ok(updated_quota) => {
                                    ed.sub_quotas.insert(account_id, updated_quota);
                                }
                                Err(err) => {
                                    ed.error_msg = Some(err);
                                }
                            }
                        }
                        Err(e) => {
                            ed.error_msg = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }
                if let Some(stat) = updated_stat {
                    app.sub_account_stats.insert(provider_id, stat);
                }
            });
        })
        .detach();
    }

    pub fn editor_save(&mut self, cx: &mut Context<Self>) {
        let locale = self.prefs.locale;
        let payload_res = {
            let Some(ref ed) = self.editor else {
                return;
            };
            if ed.saving {
                return;
            }
            ed.build_provider_payload(locale, cx)
        };

        let provider = match payload_res {
            Ok(p) => p,
            Err(msg) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.error_msg = Some(msg);
                    cx.notify();
                }
                return;
            }
        };

        if let Some(ed) = self.editor.as_mut() {
            ed.saving = true;
            ed.error_msg = None;
            save_provider_pool(&ed.provider_id, &ed.model_pool);
        }
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move { service::save_provider(&state, provider).await })
                .await;

            let _ = this.update(cx, |app, cx| match res {
                Ok(Ok(())) => {
                    app.is_provider_editor_open = false;
                    app.editor = None;
                    app.refresh_providers_only(cx);
                    cx.notify();
                }
                Ok(Err(err)) => {
                    if let Some(ed) = app.editor.as_mut() {
                        ed.saving = false;
                        ed.error_msg = Some(err);
                    }
                    cx.notify();
                }
                Err(e) => {
                    if let Some(ed) = app.editor.as_mut() {
                        ed.saving = false;
                        ed.error_msg = Some(e.to_string());
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn editor_prompt_delete_current(&mut self, cx: &mut Context<Self>) {
        if let Some(ref ed) = self.editor {
            if let Some(ref p) = ed.editing_provider {
                self.delete_provider_target = Some(p.clone());
                cx.notify();
            }
        }
    }

    pub fn execute_delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        if self.confirm_busy {
            return;
        }
        self.confirm_busy = true;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let del_id = id.clone();

        cx.spawn(async move |this, cx| {
            let _ = handle
                .spawn(async move { service::delete_provider(&state, del_id).await })
                .await;

            let _ = this.update(cx, |app, cx| {
                remove_provider_pool(&id);
                app.confirm_busy = false;
                app.delete_provider_target = None;
                if app
                    .editor
                    .as_ref()
                    .map(|e| e.provider_id == id)
                    .unwrap_or(false)
                {
                    app.is_provider_editor_open = false;
                    app.editor = None;
                }
                app.refresh_providers_only(cx);
                cx.notify();
            });
        })
        .detach();
    }

    // ================= 客户端接管与模型选择 =================

    pub fn open_client_model_select(
        &mut self,
        client_id: String,
        current_value: String,
        trigger_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = self.prefs.locale;
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(locale.pick("搜索模型...", "Search models..."))
        });
        let focus = search_input.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        let cid_enter = client_id.clone();
        cx.subscribe_in(
            &search_input,
            window,
            move |this, state, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    let raw = state.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        return;
                    }
                    let q = raw.to_lowercase();
                    let groups =
                        crate::views::clients::model_select::build_grouped_models(&this.providers);
                    let mut exact_val: Option<String> = None;
                    let mut first_val: Option<String> = None;
                    for g in &groups {
                        for opt in &g.models {
                            if opt.value.eq_ignore_ascii_case(&raw)
                                || opt.label.eq_ignore_ascii_case(&raw)
                            {
                                exact_val = Some(opt.value.clone());
                                break;
                            }
                            if first_val.is_none()
                                && (opt.label.to_lowercase().contains(&q)
                                    || opt.value.to_lowercase().contains(&q)
                                    || g.name.to_lowercase().contains(&q))
                            {
                                first_val = Some(opt.value.clone());
                            }
                        }
                        if exact_val.is_some() {
                            break;
                        }
                    }
                    if let Some(chosen) = exact_val.or(first_val) {
                        this.select_client_model(cid_enter.clone(), chosen, cx);
                    }
                }
                _ => {}
            },
        )
        .detach();

        self.client_model_select = Some(ClientModelSelectState {
            client_id,
            current_value,
            trigger_bounds,
            search_input,
            scroll_handle: ScrollHandle::new(),
        });
        cx.notify();
    }

    pub fn select_client_model(
        &mut self,
        client_id: String,
        model: String,
        cx: &mut Context<Self>,
    ) {
        self.client_model_select = None;
        self.interceptor_op_seq = self.interceptor_op_seq.wrapping_add(1);
        let seq = self.interceptor_op_seq;
        if let Some(c) = self.interceptors.iter_mut().find(|c| c.id == client_id) {
            c.model = Some(model.clone());
        }
        self.sync_tray_snapshot();
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    service::set_interceptor_model(&state, client_id, Some(model)).await
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                if app.interceptor_op_seq != seq {
                    return;
                }
                if let Ok(Ok(updated)) = res {
                    if let Some(slot) = app.interceptors.iter_mut().find(|c| c.id == updated.id) {
                        *slot = updated;
                    }
                }
                app.sync_tray_snapshot();
                cx.notify();
            });
        })
        .detach();
    }

    pub fn handle_client_switch(
        &mut self,
        client: InterceptorInfo,
        enable: bool,
        cx: &mut Context<Self>,
    ) {
        if self.toggling_interceptor_id.is_some() {
            return;
        }
        if enable && client.conflict.is_some() {
            self.conflict_target = Some(client);
            cx.notify();
            return;
        }
        self.execute_client_toggle(client, enable, cx);
    }

    pub fn confirm_conflict_takeover(&mut self, client_id: String, cx: &mut Context<Self>) {
        let Some(client) = self
            .conflict_target
            .clone()
            .or_else(|| self.interceptors.iter().find(|c| c.id == client_id).cloned())
        else {
            return;
        };
        self.conflict_target = None;
        self.execute_client_toggle(client, true, cx);
    }

    fn execute_client_toggle(
        &mut self,
        client: InterceptorInfo,
        enable: bool,
        cx: &mut Context<Self>,
    ) {
        let client_id = client.id.clone();
        let fallback_model = if enable
            && client
                .model
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            resolve_default_model_for_client(&client_id, &self.providers)
        } else {
            None
        };

        self.interceptor_op_seq = self.interceptor_op_seq.wrapping_add(1);
        let seq = self.interceptor_op_seq;
        self.toggling_interceptor_id = Some(client_id.clone());
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    if let Some(m) = fallback_model {
                        let _ =
                            service::set_interceptor_model(&state, client_id.clone(), Some(m)).await;
                    }
                    let toggle_res = service::toggle_interceptor(&state, client_id, enable).await;
                    let all = service::get_interceptor_status(&state).await;
                    (toggle_res, all)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                if app.interceptor_op_seq != seq {
                    return;
                }
                app.toggling_interceptor_id = None;
                if let Ok((toggle_res, all_res)) = res {
                    if let Err(err) = toggle_res {
                        app.global_error = Some(err);
                    }
                    if let Ok(list) = all_res {
                        app.interceptors = list;
                    }
                }
                app.sync_tray_snapshot();
                cx.notify();
            });
        })
        .detach();
    }

    // ================= 统计看板与设置页操作 =================

    pub fn prompt_clear_logs(&mut self, cx: &mut Context<Self>) {
        if !self.dashboard_stats.has_any_logs && self.logs.is_empty() {
            return;
        }
        self.pending_confirm = Some(PendingConfirm::ClearLogs);
        cx.notify();
    }

    fn execute_clear_logs(&mut self, cx: &mut Context<Self>) {
        if self.confirm_busy {
            return;
        }
        self.dashboard_req_seq = self.dashboard_req_seq.wrapping_add(1);
        self.confirm_busy = true;
        cx.notify();

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let _ = handle
                .spawn(async move { service::clear_logs(&state).await })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.logs.clear();
                app.today_summary = TodayUsageSummary::default();
                app.dashboard_stats = DashboardRawStats::default();
                app.expanded_log_id = None;
                app.hovered_chart_bucket = None;
                app.confirm_busy = false;
                app.pending_confirm = None;
                app.sync_tray_snapshot();
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_theme_preference(
        &mut self,
        theme: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prefs.theme = theme;
        save_ui_preferences(&self.prefs);
        let is_dark = ThemePalette::resolve(theme, window).is_dark;
        sync_gpui_component_theme(is_dark, Some(window), cx);
        cx.notify();
    }

    pub fn set_locale_preference(&mut self, locale: Locale, cx: &mut Context<Self>) {
        self.prefs.locale = locale;
        save_ui_preferences(&self.prefs);
        self.sync_tray_snapshot();
        cx.notify();
    }

    pub fn export_backup_json(&mut self, cx: &mut Context<Self>) {
        let locale = self.prefs.locale;
        let providers = self.providers.clone();
        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let default_filename = format!("pivot-backup-{}.json", today);
        let dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        let rx = cx.prompt_for_new_path(&dir, Some(&default_filename));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else {
                return;
            };
            let path_for_write = path.clone();
            let write_res = handle
                .spawn(async move {
                    let mut sub_accounts: Vec<SubscriptionAccount> = Vec::new();
                    for p in &providers {
                        if is_subscription_provider(p) {
                            if let Ok(accs) = state.db.get_subscription_accounts(&p.id) {
                                sub_accounts.extend(accs);
                            }
                        }
                    }
                    let payload = serde_json::json!({
                        "app": "pivot",
                        "version": 2,
                        "exportedAt": chrono::Utc::now().to_rfc3339(),
                        "providers": providers,
                        "subscriptionAccounts": sub_accounts,
                    });
                    serde_json::to_string_pretty(&payload)
                        .map_err(|e| e.to_string())
                        .and_then(|json| {
                            std::fs::write(&path_for_write, json).map_err(|e| e.to_string())
                        })
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            let _ = this.update(cx, |app, cx| {
                match write_res {
                    Ok(()) => {
                        app.settings_banner = Some(SettingsBanner {
                            is_error: false,
                            message: format!(
                                "{} {}",
                                locale.pick("配置备份已导出至", "Backup saved to"),
                                path.display()
                            ),
                        });
                    }
                    Err(err) => {
                        app.settings_banner = Some(SettingsBanner {
                            is_error: true,
                            message: format!("{}: {}", locale.pick("导出失败", "Export failed"), err),
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn import_backup_json(&mut self, cx: &mut Context<Self>) {
        let locale = self.prefs.locale;
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });

        let state = self.core_state.clone();
        let handle = self.tokio_handle.clone();

        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            let import_res = handle
                .spawn(async move {
                    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    let val: serde_json::Value =
                        serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                    let list_val = val
                        .get("providers")
                        .cloned()
                        .unwrap_or_else(|| val.clone());
                    let mut imported: Vec<Provider> =
                        serde_json::from_value(list_val).map_err(|e| e.to_string())?;

                    let imported_accounts: Vec<SubscriptionAccount> = match val.get("subscriptionAccounts") {
                        Some(acc_val) if !acc_val.is_null() => {
                            serde_json::from_value(acc_val.clone()).map_err(|e| e.to_string())?
                        }
                        _ => Vec::new(),
                    };

                    let mut imported_account_providers = HashSet::new();
                    for acc in &imported_accounts {
                        imported_account_providers.insert(acc.provider_id.clone());
                        state
                            .db
                            .save_subscription_account(acc)
                            .map_err(|e| e.to_string())?;
                    }

                    let count = imported.len();
                    for p in &mut imported {
                        p.default_model = None;
                        p.is_default = false;
                        if is_subscription_provider(p)
                            && !imported_account_providers.contains(&p.id)
                            && state
                                .db
                                .get_subscription_accounts(&p.id)
                                .map(|v| v.is_empty())
                                .unwrap_or(true)
                        {
                            p.enabled = false;
                        }
                        service::save_provider(&state, p.clone()).await?;
                    }
                    Ok::<usize, String>(count)
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                match import_res {
                    Ok(Ok(count)) => {
                        app.settings_banner = Some(SettingsBanner {
                            is_error: false,
                            message: format!(
                                "{} {} {}",
                                locale.pick("成功恢复", "Successfully restored"),
                                count,
                                locale.pick("个服务商配置", "providers")
                            ),
                        });
                        app.refresh_all(cx);
                    }
                    Ok(Err(err)) => {
                        app.settings_banner = Some(SettingsBanner {
                            is_error: true,
                            message: format!(
                                "{}: {}",
                                locale.pick("恢复备份失败", "Restore failed"),
                                err
                            ),
                        });
                    }
                    Err(e) => {
                        app.settings_banner = Some(SettingsBanner {
                            is_error: true,
                            message: e.to_string(),
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn check_for_updates_silently(&mut self, cx: &mut Context<Self>) {
        self.run_update_check(true, cx);
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.run_update_check(false, cx);
    }

    fn run_update_check(&mut self, silent: bool, cx: &mut Context<Self>) {
        if matches!(
            self.update_state,
            UpdateCheckState::Checking
                | UpdateCheckState::Downloading { .. }
                | UpdateCheckState::Installing { .. }
                | UpdateCheckState::ReadyToRestart { .. }
        ) {
            return;
        }
        self.update_check_seq = self.update_check_seq.wrapping_add(1);
        let seq = self.update_check_seq;
        if !silent {
            self.update_state = UpdateCheckState::Checking;
            cx.notify();
        }

        let client = self.core_state.http_client.clone();
        let handle = self.tokio_handle.clone();
        let handle_for_timer = self.tokio_handle.clone();
        let locale = self.prefs.locale;
        let local_ver = env!("CARGO_PKG_VERSION").to_string();

        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move { updater::fetch_latest_manifest(&client).await })
                .await;

            let mut schedule_reset = false;
            let _ = this.update(cx, |app, cx| {
                if app.update_check_seq != seq {
                    return;
                }
                match res {
                    Ok(Ok(manifest)) if !manifest.version.is_empty() => {
                        if updater::is_newer_semver(&manifest.version, &local_ver) {
                            let info = UpdateReleaseInfo {
                                version: manifest.version.clone(),
                                notes: manifest.notes,
                                asset_url: manifest.asset_url,
                                release_url: RELEASE_LATEST_PAGE_URL.to_string(),
                                app_bundle: updater::resolve_current_app_bundle(),
                            };
                            app.update_version = Some(manifest.version);
                            app.update_state = UpdateCheckState::Available(info);
                            app.sync_tray_snapshot();
                        } else if !silent {
                            app.update_version = None;
                            app.update_state = UpdateCheckState::UpToDate;
                            app.sync_tray_snapshot();
                            schedule_reset = true;
                        }
                    }
                    Ok(Ok(_)) if !silent => {
                        app.update_state = UpdateCheckState::Failed {
                            error: locale
                                .pick("远端版本信息为空", "Remote release version is empty")
                                .to_string(),
                            retry_info: None,
                        };
                    }
                    Ok(Err(err)) if !silent => {
                        app.update_state = UpdateCheckState::Failed {
                            error: err,
                            retry_info: None,
                        };
                    }
                    Err(e) if !silent => {
                        app.update_state = UpdateCheckState::Failed {
                            error: e.to_string(),
                            retry_info: None,
                        };
                    }
                    _ => {}
                }
                cx.notify();
            });

            if schedule_reset {
                let _ = handle_for_timer
                    .spawn(async move {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    })
                    .await;
                let _ = this.update(cx, |app, cx| {
                    if app.update_check_seq == seq
                        && matches!(app.update_state, UpdateCheckState::UpToDate)
                    {
                        app.update_state = UpdateCheckState::Idle;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub fn start_self_update(&mut self, info: UpdateReleaseInfo, cx: &mut Context<Self>) {
        if matches!(
            self.update_state,
            UpdateCheckState::Downloading { .. }
                | UpdateCheckState::Installing { .. }
                | UpdateCheckState::ReadyToRestart { .. }
        ) {
            return;
        }

        self.update_check_seq = self.update_check_seq.wrapping_add(1);
        self.update_state = UpdateCheckState::Downloading {
            info: info.clone(),
            progress: 0,
        };
        cx.notify();

        let client = self.core_state.http_client.clone();
        let handle = self.tokio_handle.clone();
        let locale = self.prefs.locale;
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<InstallProgressEvent>();

        let info_for_progress = info.clone();
        cx.spawn(async move |this, cx| {
            while let Some(ev) = progress_rx.recv().await {
                let info_snapshot = info_for_progress.clone();
                let ok = this
                    .update(cx, |app, cx| {
                        match ev {
                            InstallProgressEvent::Downloading(pct) => {
                                if matches!(app.update_state, UpdateCheckState::Downloading { .. }) {
                                    app.update_state = UpdateCheckState::Downloading {
                                        info: info_snapshot,
                                        progress: pct,
                                    };
                                    cx.notify();
                                }
                            }
                            InstallProgressEvent::Installing => {
                                app.update_state = UpdateCheckState::Installing {
                                    info: info_snapshot,
                                };
                                cx.notify();
                            }
                        }
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
        })
        .detach();

        let retry_snapshot = info.clone();
        let target_version = info.version.clone();
        cx.spawn(async move |this, cx| {
            let res = handle
                .spawn(async move {
                    updater::download_and_install_update(client, info, locale, progress_tx).await
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                match res {
                    Ok(Ok(app_bundle)) => {
                        app.update_state = UpdateCheckState::ReadyToRestart {
                            version: target_version,
                            app_bundle,
                        };
                        app.sync_tray_snapshot();
                    }
                    Ok(Err(err)) => {
                        app.update_state = UpdateCheckState::Failed {
                            error: err,
                            retry_info: Some(retry_snapshot),
                        };
                    }
                    Err(join_err) => {
                        app.update_state = UpdateCheckState::Failed {
                            error: join_err.to_string(),
                            retry_info: Some(retry_snapshot),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn restart_for_update(&mut self, app_bundle: PathBuf, cx: &mut Context<Self>) {
        match updater::schedule_relaunch_after_exit(&app_bundle) {
            Ok(()) => {
                tray::mark_quitting();
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_millis(1500));
                    std::process::exit(0);
                });
                cx.quit();
            }
            Err(err) => {
                self.update_state = UpdateCheckState::Failed {
                    error: format!(
                        "{}: {}",
                        self.prefs.locale.pick("自动重启失败，请手动重启应用", "Auto-restart failed, please restart manually"),
                        err
                    ),
                    retry_info: None,
                };
                cx.notify();
            }
        }
    }

    // ================= 键盘快捷键与通用确认弹窗处理 =================

    fn has_focused_editor_input(&self, window: &Window, cx: &App) -> bool {
        let Some(ref ed) = self.editor else {
            return false;
        };
        ed.preset_search_input
            .read(cx)
            .focus_handle(cx)
            .contains_focused(window, cx)
            || ed
                .name_input
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
            || ed
                .base_url_input
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
            || ed
                .model_search_input
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
            || ed
                .custom_model_input
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
            || ed
                .key_inputs
                .iter()
                .any(|k| k.read(cx).focus_handle(cx).contains_focused(window, cx))
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform;

        if key.eq_ignore_ascii_case("escape") {
            if self.delete_provider_target.is_some() {
                if !self.confirm_busy {
                    self.delete_provider_target = None;
                    cx.notify();
                }
                return;
            }
            if self.pending_confirm.is_some() {
                if !self.confirm_busy {
                    self.pending_confirm = None;
                    cx.notify();
                }
                return;
            }
            if self.conflict_target.is_some() {
                if self.toggling_interceptor_id.is_none() {
                    self.conflict_target = None;
                    cx.notify();
                }
                return;
            }
            if self.client_model_select.is_some() {
                self.client_model_select = None;
                cx.notify();
                return;
            }
            if self.context_menu.is_some() {
                self.context_menu = None;
                cx.notify();
                return;
            }
            if self.provider_drag.is_some() {
                self.provider_drag = None;
                cx.notify();
                return;
            }
            if self.active_tab == ActiveTab::Providers && self.is_provider_editor_open {
                if self.has_focused_editor_input(window, cx) {
                    self.root_focus.focus(window, cx);
                    cx.notify();
                    return;
                }
                self.editor_handle_back(window, cx);
                return;
            }
        }

        if cmd && !event.keystroke.modifiers.alt && !event.keystroke.modifiers.shift {
            let has_modal = self.delete_provider_target.is_some()
                || self.pending_confirm.is_some()
                || self.conflict_target.is_some();
            if has_modal {
                match key {
                    "w" | "W" => {
                        cx.hide();
                    }
                    "q" | "Q" => {
                        tray::mark_quitting();
                        cx.quit();
                    }
                    _ => {}
                }
                return;
            }
            match key {
                "1" => {
                    self.switch_tab(ActiveTab::Providers, cx);
                }
                "2" => {
                    self.switch_tab(ActiveTab::Clients, cx);
                }
                "3" => {
                    self.switch_tab(ActiveTab::Logs, cx);
                }
                "4" => {
                    self.switch_tab(ActiveTab::Settings, cx);
                }
                "n" | "N" => {
                    if !self.is_provider_editor_open {
                        self.open_editor(None, window, cx);
                    }
                }
                "," => {
                    self.switch_tab(ActiveTab::Settings, cx);
                }
                "w" | "W" => {
                    cx.hide();
                }
                "q" | "Q" => {
                    tray::mark_quitting();
                    cx.quit();
                }
                _ => {}
            }
        }
    }

    fn render_active_confirm_overlay(
        &self,
        palette: ThemePalette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let locale = self.prefs.locale;

        if let Some(ref target) = self.delete_provider_target {
            let del_id = target.id.clone();
            let spec = ConfirmDialogSpec {
                title: format!(
                    "{} \"{}\"?",
                    locale.pick("确认删除服务商", "Delete provider"),
                    target.name
                ),
                details: vec![locale
                    .pick(
                        "删除后将移除该服务商的凭据与模型路由配置，此操作无法撤销。",
                        "Credentials and model routing rules for this provider will be permanently removed.",
                    )
                    .to_string()],
                confirm_label: locale.pick("确认删除", "Delete").to_string(),
                cancel_label: locale.pick("取消", "Cancel").to_string(),
                variant: ConfirmVariant::Danger,
            };
            let app_cancel = cx.entity();
            return Some(render_confirm_dialog(
                &spec,
                self.confirm_busy,
                palette,
                move |_, _, app_cx| {
                    app_cancel.update(app_cx, |this, cx| {
                        this.delete_provider_target = None;
                        cx.notify();
                    });
                },
                cx.listener(move |this, _, _, cx| {
                    this.execute_delete_provider(del_id.clone(), cx);
                }),
            ));
        }

        match self.pending_confirm.as_ref()? {
            PendingConfirm::AbandonEditor(action) => {
                let action_copy = *action;
                let spec = ConfirmDialogSpec {
                    title: locale
                        .pick("放弃未保存的修改？", "Discard unsaved changes?")
                        .to_string(),
                    details: vec![locale
                        .pick(
                            "当前服务商表单包含未保存的改动，离开后这些修改将会丢失。",
                            "Your current edits have not been saved and will be lost.",
                        )
                        .to_string()],
                    confirm_label: locale.pick("放弃修改", "Discard").to_string(),
                    cancel_label: locale.pick("继续编辑", "Keep Editing").to_string(),
                    variant: ConfirmVariant::Danger,
                };
                let app_cancel = cx.entity();
                Some(render_confirm_dialog(
                    &spec,
                    false,
                    palette,
                    move |_, _, app_cx| {
                        app_cancel.update(app_cx, |this, cx| {
                            this.pending_confirm = None;
                            cx.notify();
                        });
                    },
                    cx.listener(move |this, _, _, cx| {
                        this.pending_confirm = None;
                        match action_copy {
                            AbandonAction::Leave => {
                                this.close_editor_immediately(cx);
                            }
                            AbandonAction::BackToPresets => {
                                if let Some(ed) = this.editor.as_mut() {
                                    ed.choosing_preset = true;
                                    ed.error_msg = None;
                                }
                                cx.notify();
                            }
                            AbandonAction::SwitchTab(tab) => {
                                this.is_provider_editor_open = false;
                                this.editor = None;
                                this.switch_tab(tab, cx);
                            }
                        }
                    }),
                ))
            }
            PendingConfirm::ClearLogs => {
                let spec = ConfirmDialogSpec {
                    title: locale
                        .pick("清空全部请求统计记录？", "Clear all request logs?")
                        .to_string(),
                    details: vec![locale
                        .pick(
                            "将永久清除本地数据库中的所有历史请求、Token 与费用统计数据。",
                            "All historical request, token, and cost logs in ~/.pivot/pivot.db will be cleared.",
                        )
                        .to_string()],
                    confirm_label: locale.pick("确认清空", "Clear All").to_string(),
                    cancel_label: locale.pick("取消", "Cancel").to_string(),
                    variant: ConfirmVariant::Danger,
                };
                let app_cancel = cx.entity();
                Some(render_confirm_dialog(
                    &spec,
                    self.confirm_busy,
                    palette,
                    move |_, _, app_cx| {
                        app_cancel.update(app_cx, |this, cx| {
                            this.pending_confirm = None;
                            cx.notify();
                        });
                    },
                    cx.listener(|this, _, _, cx| {
                        this.execute_clear_logs(cx);
                    }),
                ))
            }
        }
    }
}

impl Render for PivotApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_open_add_provider {
            self.pending_open_add_provider = false;
            if !self.is_provider_editor_open {
                self.open_editor(None, window, cx);
            }
        }

        let palette = ThemePalette::resolve(self.prefs.theme, window);
        sync_gpui_component_theme(palette.is_dark, Some(window), cx);
        tray::sync_macos_app_icon(palette.is_dark);

        let active_tab = self.active_tab;
        let locale = self.prefs.locale;

        // 计算拖拽浮动行快照
        let drag_preview_data: Option<(Provider, f32)> = self
            .provider_drag
            .as_ref()
            .filter(|d| d.active)
            .and_then(|d| {
                self.providers
                    .iter()
                    .find(|p| p.id == d.id)
                    .cloned()
                    .map(|p| (p, d.pointer_y))
            });

        let main_body: AnyElement = match active_tab {
            ActiveTab::Providers => {
                if self.is_provider_editor_open {
                    if let Some(ed_ref) = self.editor.as_ref() {
                        // 为满足借用检查器，直接传入不可变借用渲染
                        let ed_ptr: *const ProviderEditorState = ed_ref;
                        let elem = unsafe {
                            render_provider_editor(self, &*ed_ptr, palette, window, cx)
                        };
                        elem.into_any_element()
                    } else {
                        render_provider_list(self, palette, window, cx).into_any_element()
                    }
                } else {
                    render_provider_list(self, palette, window, cx).into_any_element()
                }
            }
            ActiveTab::Clients => render_clients_view(self, palette, window, cx).into_any_element(),
            ActiveTab::Logs => render_dashboard_view(self, palette, window, cx).into_any_element(),
            ActiveTab::Settings => {
                render_settings_view(self, palette, window, cx).into_any_element()
            }
        };

        let ctx_menu_el: Option<AnyElement> = self.context_menu.as_ref().map(|m| {
            let m_ptr: *const ProviderContextMenuState = m;
            unsafe { render_provider_context_menu(self, &*m_ptr, palette, window, cx) }
                .into_any_element()
        });

        let model_pop_el: Option<AnyElement> = self.client_model_select.as_ref().map(|s| {
            let s_ptr: *const ClientModelSelectState = s;
            unsafe { render_client_model_popover(self, &*s_ptr, palette, window, cx) }
        });

        let conflict_el: Option<AnyElement> = self.conflict_target.clone().map(|target| {
            let busy = self.toggling_interceptor_id.is_some();
            render_conflict_dialog(self, &target, busy, palette, cx)
        });

        let confirm_el: Option<AnyElement> = self.render_active_confirm_overlay(palette, cx);

        div()
            .id("pivot_root")
            .track_focus(&self.root_focus)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.background)
            .text_color(palette.foreground)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                this.handle_key_down(e, window, cx);
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                this.handle_root_mouse_move(e, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.handle_root_mouse_up(cx);
                }),
            )
            // 1. 48px 原生感顶栏（含红绿灯避让与拖拽区）
            .child(render_header(self, palette, window, cx))
            // 2. 全局非阻塞错误横幅
            .when_some(self.global_error.clone(), |el, err| {
                el.child(
                    div()
                        .mx(px(16.0))
                        .mt(px(10.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(6.0))
                        .bg(palette.rose_soft_bg)
                        .border_1()
                        .border_color(palette.rose_soft_border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.5))
                                .text_color(palette.rose_text)
                                .child(err),
                        )
                        .child(
                            div()
                                .id("dismiss_global_err")
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.global_error = None;
                                        cx.notify();
                                    }),
                                )
                                .child(lucide_icon(IconName::X, 12.0, palette.rose_text)),
                        ),
                )
            })
            // 3. 主滚动视图区（无滚动条轨道、保留原生触控板/滚轮惯性滚动；宽窗自动居中 max-w 680px）
            .child(
                div()
                    .id("pivot_main_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll_handle)
                    .px(px(16.0))
                    .py(px(14.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(div().w_full().max_w(px(680.0)).child(main_body)),
            )
            // 4. 服务商拖拽跟随浮动预览卡
            .when_some(drag_preview_data, |el, (prov, pointer_y)| {
                let top_y = (pointer_y - PROVIDER_ROW_HEIGHT * 0.5).max(52.0);
                el.child(
                    div()
                        .absolute()
                        .left(px(16.0))
                        .right(px(16.0))
                        .top(px(top_y))
                        .rounded(px(8.0))
                        .bg(palette.popover)
                        .border_1()
                        .border_color(palette.ring_border)
                        .shadow(palette.popover_shadows())
                        .overflow_hidden()
                        .opacity(0.96)
                        .child(render_provider_row(
                            self, &prov, 0, false, true, locale, palette, cx,
                        )),
                )
            })
            // 5. 右键上下文菜单浮层
            .when_some(ctx_menu_el, |el, menu| el.child(menu))
            // 6. 客户端模型搜索下拉浮层
            .when_some(model_pop_el, |el, pop| el.child(pop))
            // 7. 客户端代理冲突解决模态框
            .when_some(conflict_el, |el, modal| el.child(modal))
            // 8. 最高优先级通用二次确认对话框
            .when_some(confirm_el, |el, dlg| el.child(dlg))
    }
}
