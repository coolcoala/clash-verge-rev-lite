use once_cell::sync::OnceCell;
use tauri::tray::TrayIconBuilder;
#[cfg(target_os = "macos")]
pub mod speed_rate;
use crate::{
    config::Config,
    feat, logging,
    module::{lightweight::is_in_lightweight_mode, mihomo::Rate},
    utils::{dirs::find_target_icons, i18n::t, resolve::VERSION},
    Type,
};

use anyhow::Result;
use parking_lot::Mutex;
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Wry,
};

use super::handle;

#[derive(Clone)]
struct TrayState {}

// 托盘点击防抖机制
static TRAY_CLICK_DEBOUNCE: OnceCell<Mutex<Instant>> = OnceCell::new();
const TRAY_CLICK_DEBOUNCE_MS: u64 = 300;

fn get_tray_click_debounce() -> &'static Mutex<Instant> {
    TRAY_CLICK_DEBOUNCE.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(1)))
}

fn should_handle_tray_click() -> bool {
    let debounce_lock = get_tray_click_debounce();
    let mut last_click = debounce_lock.lock();
    let now = Instant::now();

    if now.duration_since(*last_click) >= Duration::from_millis(TRAY_CLICK_DEBOUNCE_MS) {
        *last_click = now;
        true
    } else {
        log::debug!(target: "app", "Tray click ignored by debounce; time since last click: {:?}ms", 
                  now.duration_since(*last_click).as_millis());
        false
    }
}

#[cfg(target_os = "macos")]
pub struct Tray {
    last_menu_update: Mutex<Option<Instant>>,
    menu_updating: AtomicBool,
}

#[cfg(not(target_os = "macos"))]
pub struct Tray {
    last_menu_update: Mutex<Option<Instant>>,
    menu_updating: AtomicBool,
}

impl TrayState {
    pub fn get_common_tray_icon() -> (bool, Vec<u8>) {
        let verge = Config::verge().latest().clone();
        let is_common_tray_icon = verge.common_tray_icon.unwrap_or(false);
        if is_common_tray_icon {
            if let Some(common_icon_path) = find_target_icons("common").unwrap() {
                let icon_data = fs::read(common_icon_path).unwrap();
                return (true, icon_data);
            }
        }
        #[cfg(target_os = "macos")]
        {
            let tray_icon_colorful = verge.tray_icon.unwrap_or("monochrome".to_string());
            if tray_icon_colorful == "monochrome" {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon-mono.ico").to_vec(),
                )
            } else {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon.ico").to_vec(),
                )
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            (
                false,
                include_bytes!("../../../icons/tray-icon.ico").to_vec(),
            )
        }
    }

    pub fn get_sysproxy_tray_icon() -> (bool, Vec<u8>) {
        let verge = Config::verge().latest().clone();
        let is_sysproxy_tray_icon = verge.sysproxy_tray_icon.unwrap_or(false);
        if is_sysproxy_tray_icon {
            if let Some(sysproxy_icon_path) = find_target_icons("sysproxy").unwrap() {
                let icon_data = fs::read(sysproxy_icon_path).unwrap();
                return (true, icon_data);
            }
        }
        #[cfg(target_os = "macos")]
        {
            let tray_icon_colorful = verge.tray_icon.clone().unwrap_or("monochrome".to_string());
            if tray_icon_colorful == "monochrome" {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon-sys-mono-new.ico").to_vec(),
                )
            } else {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon-sys.ico").to_vec(),
                )
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            (
                false,
                include_bytes!("../../../icons/tray-icon-sys.ico").to_vec(),
            )
        }
    }

    pub fn get_tun_tray_icon() -> (bool, Vec<u8>) {
        let verge = Config::verge().latest().clone();
        let is_tun_tray_icon = verge.tun_tray_icon.unwrap_or(false);
        if is_tun_tray_icon {
            if let Some(tun_icon_path) = find_target_icons("tun").unwrap() {
                let icon_data = fs::read(tun_icon_path).unwrap();
                return (true, icon_data);
            }
        }
        #[cfg(target_os = "macos")]
        {
            let tray_icon_colorful = verge.tray_icon.clone().unwrap_or("monochrome".to_string());
            if tray_icon_colorful == "monochrome" {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon-tun-mono-new.ico").to_vec(),
                )
            } else {
                (
                    false,
                    include_bytes!("../../../icons/tray-icon-tun.ico").to_vec(),
                )
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            (
                false,
                include_bytes!("../../../icons/tray-icon-tun.ico").to_vec(),
            )
        }
    }
}

impl Tray {
    pub fn global() -> &'static Tray {
        static TRAY: OnceCell<Tray> = OnceCell::new();

        #[cfg(target_os = "macos")]
        return TRAY.get_or_init(|| Tray {
            last_menu_update: Mutex::new(None),
            menu_updating: AtomicBool::new(false),
        });

        #[cfg(not(target_os = "macos"))]
        return TRAY.get_or_init(|| Tray {
            last_menu_update: Mutex::new(None),
            menu_updating: AtomicBool::new(false),
        });
    }

    pub fn init(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray initialization");
            return Ok(());
        }
        Ok(())
    }

    /// 更新托盘点击行为
    pub fn update_click_behavior(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray click behavior update");
            return Ok(());
        }
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let tray_event = { Config::verge().latest().tray_event.clone() };
        let tray_event: String = tray_event.unwrap_or("main_window".into());
        let tray = app_handle.tray_by_id("main").unwrap();
        match tray_event.as_str() {
            "tray_menu" => tray.set_show_menu_on_left_click(true)?,
            _ => tray.set_show_menu_on_left_click(false)?,
        }
        Ok(())
    }

    /// 更新托盘菜单
    pub fn update_menu(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray menu update");
            return Ok(());
        }
        // 调整最小更新间隔，确保状态及时刷新
        const MIN_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

        // 检查是否正在更新
        if self.menu_updating.load(Ordering::Acquire) {
            return Ok(());
        }

        // 检查更新频率，但允许重要事件跳过频率限制
        let should_force_update = match std::thread::current().name() {
            Some("main") => true,
            _ => {
                let last_update = self.last_menu_update.lock();
                if let Some(last_time) = *last_update {
                    last_time.elapsed() >= MIN_UPDATE_INTERVAL
                } else {
                    true
                }
            }
        };

        if !should_force_update {
            return Ok(());
        }

        let app_handle = match handle::Handle::global().app_handle() {
            Some(handle) => handle,
            None => {
                log::warn!(target: "app", "Failed to update tray menu: app_handle not found");
                return Ok(());
            }
        };

        // 设置更新状态
        self.menu_updating.store(true, Ordering::Release);

        let result = self.update_menu_internal(&app_handle);

        {
            let mut last_update = self.last_menu_update.lock();
            *last_update = Some(Instant::now());
        }
        self.menu_updating.store(false, Ordering::Release);

        result
    }

    fn update_menu_internal(&self, app_handle: &AppHandle) -> Result<()> {
        let verge = Config::verge().latest().clone();
        let system_proxy = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);
        let mode = {
            Config::clash()
                .latest()
                .0
                .get("mode")
                .map(|val| val.as_str().unwrap_or("rule"))
                .unwrap_or("rule")
                .to_owned()
        };
        let profile_uid_and_name = Config::profiles()
            .data()
            .all_profile_uid_and_name()
            .unwrap_or_default();
        let is_lightweight_mode = is_in_lightweight_mode();

        match app_handle.tray_by_id("main") {
            Some(tray) => {
                let _ = tray.set_menu(Some(create_tray_menu(
                    app_handle,
                    Some(mode.as_str()),
                    *system_proxy,
                    *tun_mode,
                    profile_uid_and_name,
                    is_lightweight_mode,
                )?));
                log::debug!(target: "app", "Tray menu updated successfully");
                Ok(())
            }
            None => {
                log::warn!(target: "app", "Failed to update tray menu: tray not found");
                Ok(())
            }
        }
    }

    /// 更新托盘图标
    #[cfg(target_os = "macos")]
    pub fn update_icon(&self, _rate: Option<Rate>) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray icon update");
            return Ok(());
        }
        let app_handle = match handle::Handle::global().app_handle() {
            Some(handle) => handle,
            None => {
                log::warn!(target: "app", "Failed to update tray icon: app_handle not found");
                return Ok(());
            }
        };

        let tray = match app_handle.tray_by_id("main") {
            Some(tray) => tray,
            None => {
                log::warn!(target: "app", "Failed to update tray icon: tray not found");
                return Ok(());
            }
        };

        let verge = Config::verge().latest().clone();
        let system_mode = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let (_is_custom_icon, icon_bytes) = match (*system_mode, *tun_mode) {
            (true, true) => TrayState::get_tun_tray_icon(),
            (true, false) => TrayState::get_sysproxy_tray_icon(),
            (false, true) => TrayState::get_tun_tray_icon(),
            (false, false) => TrayState::get_common_tray_icon(),
        };

        let colorful = verge.tray_icon.clone().unwrap_or("monochrome".to_string());
        let is_colorful = colorful == "colorful";

        let _ = tray.set_icon(Some(tauri::image::Image::from_bytes(&icon_bytes)?));
        let _ = tray.set_icon_as_template(!is_colorful);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn update_icon(&self, _rate: Option<Rate>) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray icon update");
            return Ok(());
        }
        let app_handle = match handle::Handle::global().app_handle() {
            Some(handle) => handle,
            None => {
                log::warn!(target: "app", "Failed to update tray icon: app_handle not found");
                return Ok(());
            }
        };

        let tray = match app_handle.tray_by_id("main") {
            Some(tray) => tray,
            None => {
                log::warn!(target: "app", "Failed to update tray icon: tray not found");
                return Ok(());
            }
        };

        let verge = Config::verge().latest().clone();
        let system_mode = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let (_is_custom_icon, icon_bytes) = match (*system_mode, *tun_mode) {
            (true, true) => TrayState::get_tun_tray_icon(),
            (true, false) => TrayState::get_sysproxy_tray_icon(),
            (false, true) => TrayState::get_tun_tray_icon(),
            (false, false) => TrayState::get_common_tray_icon(),
        };

        let _ = tray.set_icon(Some(tauri::image::Image::from_bytes(&icon_bytes)?));
        Ok(())
    }

    /// 更新托盘显示状态的函数
    pub fn update_tray_display(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray display update");
            return Ok(());
        }
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let _tray = app_handle.tray_by_id("main").unwrap();

        // 更新菜单
        self.update_menu()?;

        Ok(())
    }

    /// 更新托盘提示
    pub fn update_tooltip(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray tooltip update");
            return Ok(());
        }
        let app_handle = match handle::Handle::global().app_handle() {
            Some(handle) => handle,
            None => {
                log::warn!(target: "app", "Failed to update tray tooltip: app_handle not found");
                return Ok(());
            }
        };

        let version = match VERSION.get() {
            Some(v) => v,
            None => {
                log::warn!(target: "app", "Failed to update tray tooltip: version info not found");
                return Ok(());
            }
        };

        let verge = Config::verge().latest().clone();
        let system_proxy = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let switch_map = {
            let mut map = std::collections::HashMap::new();
            map.insert(true, "on");
            map.insert(false, "off");
            map
        };

        let mut current_profile_name = "None".to_string();
        let profiles = Config::profiles();
        let profiles = profiles.latest();
        if let Some(current_profile_uid) = profiles.get_current() {
            if let Ok(profile) = profiles.get_item(&current_profile_uid) {
                current_profile_name = match &profile.name {
                    Some(profile_name) => profile_name.to_string(),
                    None => current_profile_name,
                };
            }
        };

        if let Some(tray) = app_handle.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(&format!(
                "Koala Clash {version}\n{}: {}\n{}: {}\n{}: {}",
                t("SysProxy"),
                switch_map[system_proxy],
                t("TUN"),
                switch_map[tun_mode],
                t("Profile"),
                current_profile_name
            )));
        } else {
            log::warn!(target: "app", "Failed to update tray tooltip: tray not found");
        }

        Ok(())
    }

    pub fn update_part(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray partial update");
            return Ok(());
        }
        self.update_menu()?;
        self.update_icon(None)?;
        self.update_tooltip()?;
        // 更新轻量模式显示状态
        self.update_tray_display()?;
        Ok(())
    }

    /// 取消订阅 traffic 数据
    #[cfg(target_os = "macos")]
    pub fn unsubscribe_traffic(&self) {}

    pub fn create_tray_from_handle(&self, app_handle: &AppHandle) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray creation");
            return Ok(());
        }
        log::info!(target: "app", "Creating system tray from AppHandle");

        // 获取图标
        let icon_bytes = TrayState::get_common_tray_icon().1;
        let icon = tauri::image::Image::from_bytes(&icon_bytes)?;

        #[cfg(target_os = "linux")]
        let builder = TrayIconBuilder::with_id("main")
            .icon(icon)
            .icon_as_template(false);

        #[cfg(not(target_os = "linux"))]
        let mut builder = TrayIconBuilder::with_id("main")
            .icon(icon)
            .icon_as_template(false);

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let tray_event = { Config::verge().latest().tray_event.clone() };
            let tray_event: String = tray_event.unwrap_or("main_window".into());
            if tray_event.as_str() != "tray_menu" {
                builder = builder.show_menu_on_left_click(false);
            }
        }

        let tray = builder.build(app_handle)?;

        tray.on_tray_icon_event(|_, event| {
            let tray_event = { Config::verge().latest().tray_event.clone() };
            let tray_event: String = tray_event.unwrap_or("main_window".into());
            log::debug!(target: "app","tray event: {tray_event:?}");

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                // 添加防抖检查，防止快速连击
                if !should_handle_tray_click() {
                    return;
                }

                match tray_event.as_str() {
                    "system_proxy" => feat::toggle_system_proxy(),
                    "tun_mode" => feat::toggle_tun_mode(None),
                    "main_window" => {
                        use crate::utils::window_manager::WindowManager;
                        log::info!(target: "app", "Tray click: show main window");
                        if crate::module::lightweight::is_in_lightweight_mode() {
                            log::info!(target: "app", "Currently in lightweight mode, exiting lightweight mode");
                            crate::module::lightweight::exit_lightweight_mode();
                        }
                        let result = WindowManager::show_main_window();
                        log::info!(target: "app", "Window show result: {result:?}");
                    }
                    _ => {}
                }
            }
        });
        tray.on_menu_event(on_menu_event);
        log::info!(target: "app", "System tray created successfully");
        Ok(())
    }

    // 托盘统一的状态更新函数
    pub fn update_all_states(&self) -> Result<()> {
        if handle::Handle::global().is_exiting() {
            log::debug!(target: "app", "Application is exiting, skip tray state update");
            return Ok(());
        }
        // 确保所有状态更新完成
        self.update_menu()?;
        self.update_icon(None)?;
        self.update_tooltip()?;
        self.update_tray_display()?;

        Ok(())
    }
}

fn create_tray_menu(
    app_handle: &AppHandle,
    mode: Option<&str>,
    system_proxy_enabled: bool,
    tun_mode_enabled: bool,
    profile_uid_and_name: Vec<(String, String)>,
    is_lightweight_mode: bool,
) -> Result<tauri::menu::Menu<Wry>> {
    let mode = mode.unwrap_or("");

    let unknown_version = String::from("unknown");
    let version = VERSION.get().unwrap_or(&unknown_version);

    let hotkeys = Config::verge()
        .latest()
        .hotkeys
        .as_ref()
        .map(|h| {
            h.iter()
                .filter_map(|item| {
                    let mut parts = item.split(',');
                    match (parts.next(), parts.next()) {
                        (Some(func), Some(key)) => Some((func.to_string(), key.to_string())),
                        _ => None,
                    }
                })
                .collect::<std::collections::HashMap<String, String>>()
        })
        .unwrap_or_default();

    let profile_menu_items: Vec<CheckMenuItem<Wry>> = profile_uid_and_name
        .iter()
        .map(|(profile_uid, profile_name)| {
            let is_current_profile = Config::profiles()
                .data()
                .is_current_profile_index(profile_uid.to_string());
            CheckMenuItem::with_id(
                app_handle,
                format!("profiles_{profile_uid}"),
                t(profile_name),
                true,
                is_current_profile,
                None::<&str>,
            )
            .unwrap()
        })
        .collect();
    let profile_menu_items: Vec<&dyn IsMenuItem<Wry>> = profile_menu_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();

    let open_window = &MenuItem::with_id(
        app_handle,
        "open_window",
        t("Dashboard"),
        true,
        hotkeys.get("open_or_close_dashboard").map(|s| s.as_str()),
    )
    .unwrap();

    let rule_mode = &CheckMenuItem::with_id(
        app_handle,
        "rule_mode",
        t("Rule Mode"),
        true,
        mode == "rule",
        hotkeys.get("clash_mode_rule").map(|s| s.as_str()),
    )
    .unwrap();

    let global_mode = &CheckMenuItem::with_id(
        app_handle,
        "global_mode",
        t("Global Mode"),
        true,
        mode == "global",
        hotkeys.get("clash_mode_global").map(|s| s.as_str()),
    )
    .unwrap();

    let profiles = &Submenu::with_id_and_items(
        app_handle,
        "profiles",
        t("Profiles"),
        true,
        &profile_menu_items,
    )
    .unwrap();

    let system_proxy = &CheckMenuItem::with_id(
        app_handle,
        "system_proxy",
        t("System Proxy"),
        true,
        system_proxy_enabled,
        hotkeys.get("toggle_system_proxy").map(|s| s.as_str()),
    )
    .unwrap();

    let tun_mode = &CheckMenuItem::with_id(
        app_handle,
        "tun_mode",
        t("TUN Mode"),
        true,
        tun_mode_enabled,
        hotkeys.get("toggle_tun_mode").map(|s| s.as_str()),
    )
    .unwrap();

    let lighteweight_mode = &CheckMenuItem::with_id(
        app_handle,
        "entry_lightweight_mode",
        t("LightWeight Mode"),
        true,
        is_lightweight_mode,
        hotkeys.get("entry_lightweight_mode").map(|s| s.as_str()),
    )
    .unwrap();

    let restart_clash = &MenuItem::with_id(
        app_handle,
        "restart_clash",
        t("Restart Clash Core"),
        true,
        None::<&str>,
    )
    .unwrap();

    let restart_app = &MenuItem::with_id(
        app_handle,
        "restart_app",
        t("Restart App"),
        true,
        None::<&str>,
    )
    .unwrap();

    let app_version = &MenuItem::with_id(
        app_handle,
        "app_version",
        format!("{} {version}", t("Verge Version")),
        true,
        None::<&str>,
    )
    .unwrap();

    let more = &Submenu::with_id_and_items(
        app_handle,
        "more",
        t("More"),
        true,
        &[restart_clash, restart_app, app_version],
    )
    .unwrap();

    let quit =
        &MenuItem::with_id(app_handle, "quit", t("Exit"), true, Some("CmdOrControl+Q")).unwrap();

    let separator = &PredefinedMenuItem::separator(app_handle).unwrap();

    let menu = tauri::menu::MenuBuilder::new(app_handle)
        .items(&[
            open_window,
            separator,
            rule_mode,
            global_mode,
            separator,
            profiles,
            separator,
            system_proxy,
            tun_mode,
            separator,
            lighteweight_mode,
            more,
            separator,
            quit,
        ])
        .build()
        .unwrap();
    Ok(menu)
}

fn on_menu_event(_: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        mode @ ("rule_mode" | "global_mode" | "direct_mode") => {
            let mode = &mode[0..mode.len() - 5];
            logging!(
                info,
                Type::ProxyMode,
                true,
                "Switch Proxy Mode To: {}",
                mode
            );
            feat::change_clash_mode(mode.into());
        }
        "open_window" => {
            use crate::utils::window_manager::WindowManager;
            log::info!(target: "app", "Tray menu click: open window");

            if !should_handle_tray_click() {
                return;
            }

            if crate::module::lightweight::is_in_lightweight_mode() {
                log::info!(target: "app", "Currently in lightweight mode, exiting");
                crate::module::lightweight::exit_lightweight_mode();
            }
            let result = WindowManager::show_main_window();
            log::info!(target: "app", "Window show result: {result:?}");
        }
        "system_proxy" => {
            feat::toggle_system_proxy();
        }
        "tun_mode" => {
            feat::toggle_tun_mode(None);
        }
        "restart_clash" => feat::restart_clash_core(),
        "restart_app" => feat::restart_app(),
        "entry_lightweight_mode" => {
            if !should_handle_tray_click() {
                return;
            }

            let was_lightweight = crate::module::lightweight::is_in_lightweight_mode();
            if was_lightweight {
                crate::module::lightweight::exit_lightweight_mode();
            } else {
                crate::module::lightweight::entry_lightweight_mode();
            }

            if was_lightweight {
                use crate::utils::window_manager::WindowManager;
                let result = WindowManager::show_main_window();
                log::info!(target: "app", "Show main window after exiting lightweight mode: {result:?}");
            }
        }
        "quit" => {
            feat::quit();
        }
        id if id.starts_with("profiles_") => {
            let profile_index = &id["profiles_".len()..];
            feat::toggle_proxy_profile(profile_index.into());
        }
        _ => {}
    }

    if let Err(e) = Tray::global().update_all_states() {
        log::warn!(target: "app", "Failed to update tray state: {e}");
    }
}
