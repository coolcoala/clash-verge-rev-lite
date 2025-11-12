mod cmd;
mod config;
mod core;
mod enhance;
mod feat;
mod module;
mod process;
mod state;
mod utils;
use crate::{
    core::{event_driven_proxy::EventDrivenProxyManager, hotkey},
    process::AsyncHandler,
    utils::resolve,
};
use config::Config;
use std::sync::{Mutex, Once};
use tauri::AppHandle;
use tauri::Manager;
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::time::{timeout, Duration};
use utils::logging::Type;

/// A global singleton handle to the application.
pub struct AppHandleManager {
    inner: Mutex<Option<AppHandle>>,
    init: Once,
}

impl AppHandleManager {
    /// Get the global instance of the app handle manager.
    pub fn global() -> &'static Self {
        static INSTANCE: AppHandleManager = AppHandleManager {
            inner: Mutex::new(None),
            init: Once::new(),
        };
        &INSTANCE
    }

    /// Initialize the app handle manager with an app handle.
    pub fn init(&self, handle: AppHandle) {
        self.init.call_once(|| {
            let mut app_handle = self.inner.lock().unwrap();
            *app_handle = Some(handle);
        });
    }

    /// Get the app handle if it has been initialized.
    pub fn get(&self) -> Option<AppHandle> {
        self.inner.lock().unwrap().clone()
    }

    /// Get the app handle, panics if it hasn't been initialized.
    pub fn get_handle(&self) -> AppHandle {
        self.get().expect("AppHandle not initialized")
    }

    pub fn set_activation_policy_regular(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Regular);
        }
    }

    pub fn set_activation_policy_accessory(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }

    pub fn set_activation_policy_prohibited(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Prohibited);
        }
    }
}

#[allow(clippy::panic)]
pub fn run() {
	// Capture early deep link before any async setup (cold start on macOS)
	utils::resolve::capture_early_deep_link_from_args();

	utils::network::NetworkManager::global().init();

    let _ = utils::dirs::init_portable_flag();

    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    #[cfg(debug_assertions)]
    let devtools = tauri_plugin_devtools::init();

	#[allow(unused_mut)]
	let mut builder = tauri::Builder::default()
		.plugin(tauri_plugin_single_instance::init(|_app, argv, _cwd| {
			// When a second instance is invoked, always show the window
			AsyncHandler::spawn(move || async move {
				// Exit lightweight mode if active
				if crate::module::lightweight::is_in_lightweight_mode() {
					logging!(info, Type::System, true, "Second instance detected: exiting lightweight mode");
					crate::module::lightweight::exit_lightweight_mode();
					// Wait for lightweight mode to fully exit
					for _ in 0..50 {
						if !crate::module::lightweight::is_in_lightweight_mode() {
							break;
						}
						tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
					}
				}
				
				// Show the main window
				logging!(info, Type::System, true, "Second instance detected: showing main window");
				let _ = crate::utils::window_manager::WindowManager::show_main_window();
				
				// Handle deep link if present
				if let Some(url) = argv
					.iter()
					.find(|a| a.starts_with("clash://") || a.starts_with("koala-clash://"))
					.cloned()
				{
					logging!(info, Type::System, true, "Second instance with deep link: {}", url);
					resolve::schedule_handle_deep_link(url);
				}
			});
		}))
		.plugin(tauri_plugin_notification::init())
		.plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(tauri_plugin_clipboard_manager::init())
		.plugin(tauri_plugin_process::init())
		.plugin(tauri_plugin_global_shortcut::Builder::new().build())
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_shell::init())
		.plugin(tauri_plugin_deep_link::init())
		.setup(|app| {
			logging!(info, Type::Setup, true, "Starting app initialization...");

			// Register deep link handler as early as possible to not miss cold-start events (macOS)
			app.deep_link().on_open_url(|event| {
				let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
				logging!(info, Type::Setup, true, "on_open_url received: {:?}", urls);
				if let Some(url) = urls.first().cloned() {
					resolve::schedule_handle_deep_link(url);
				}
			});

			let mut auto_start_plugin_builder = tauri_plugin_autostart::Builder::new();
			#[cfg(target_os = "macos")]
			{
				auto_start_plugin_builder = auto_start_plugin_builder
					.macos_launcher(MacosLauncher::LaunchAgent)
					.app_name(app.config().identifier.clone());
			}
			let _ = app.handle().plugin(auto_start_plugin_builder.build());

			// Ensure URL schemes are registered with the OS (all platforms)
			logging!(info, Type::Setup, true, "Registering deep links with OS...");
			logging_error!(Type::System, true, app.deep_link().register_all());

			// Deep link handler will be registered AFTER core handle init to ensure window creation works

            // 窗口管理
            logging!(
                info,
                Type::Setup,
                true,
                "Initializing window state management..."
            );
            let window_state_plugin = tauri_plugin_window_state::Builder::new()
                .with_filename("window_state.json")
                .with_state_flags(tauri_plugin_window_state::StateFlags::default())
                .build();
            let _ = app.handle().plugin(window_state_plugin);

            // 异步处理
            let app_handle = app.handle().clone();
            AsyncHandler::spawn(move || async move {
                logging!(
                    info,
                    Type::Setup,
                    true,
                    "Executing app setup asynchronously..."
                );
                match timeout(
                    Duration::from_secs(30),
                    resolve::resolve_setup_async(&app_handle),
                )
                .await
                {
                    Ok(_) => {
                        logging!(info, Type::Setup, true, "App setup completed successfully");
                    }
                    Err(_) => {
                        logging!(
                            error,
                            Type::Setup,
                            true,
                            "App setup timed out (30s), continuing with subsequent steps"
                        );
                    }
                }
            });

            logging!(
                info,
                Type::Setup,
                true,
                "Executing main setup operations..."
            );

            logging!(info, Type::Setup, true, "Initializing AppHandleManager...");
            AppHandleManager::global().init(app.handle().clone());

            logging!(info, Type::Setup, true, "Initializing core handle...");
            core::handle::Handle::global().init(app.handle());

            logging!(info, Type::Setup, true, "Initializing config...");
            if let Err(e) = utils::init::init_config() {
                logging!(
                    error,
                    Type::Setup,
                    true,
                    "Failed to initialize config: {}",
                    e
                );
            }

            logging!(info, Type::Setup, true, "Initializing resources...");
            if let Err(e) = utils::init::init_resources() {
                logging!(
                    error,
                    Type::Setup,
                    true,
                    "Failed to initialize resources: {}",
                    e
                );
            }

            app.manage(Mutex::new(state::proxy::CmdProxyState::default()));
            app.manage(Mutex::new(state::lightweight::LightWeightState::default()));

			// If an early deep link was captured from argv, schedule it now (after core and window can be created)
			utils::resolve::replay_early_deep_link();

			// (deep link handler already registered above)

			tauri::async_runtime::spawn(async {
				tokio::time::sleep(Duration::from_secs(5)).await;
				logging!(
					info,
					Type::Cmd,
					true,
					"Running profile updates at startup..."
				);
				if let Err(e) = crate::cmd::update_profiles_on_startup().await {
					log::error!("Failed to update profiles on startup: {e}");
				}
			});

            logging!(
                info,
                Type::Setup,
                true,
                "Initialization completed, continuing"
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // common
            cmd::get_sys_proxy,
            cmd::get_auto_proxy,
            cmd::open_app_dir,
            cmd::open_logs_dir,
            cmd::open_web_url,
            cmd::open_core_dir,
            cmd::get_portable_flag,
            cmd::get_network_interfaces,
            cmd::get_system_hostname,
            cmd::restart_app,
            // 内核管理
            cmd::start_core,
            cmd::stop_core,
            cmd::restart_core,
            // 启动命令
            cmd::notify_ui_ready,
            cmd::update_ui_stage,
            cmd::reset_ui_ready_state,
            cmd::get_running_mode,
            cmd::get_app_uptime,
            cmd::get_auto_launch_status,
            cmd::is_admin,
            // 添加轻量模式相关命令
            cmd::entry_lightweight_mode,
            cmd::exit_lightweight_mode,
            // service 管理
            cmd::install_service,
            cmd::uninstall_service,
            cmd::reinstall_service,
            cmd::repair_service,
            cmd::is_service_available,
            // clash
            cmd::get_clash_info,
            cmd::patch_clash_config,
            cmd::patch_clash_mode,
            cmd::change_clash_core,
            cmd::get_runtime_config,
            cmd::get_runtime_yaml,
            cmd::get_runtime_exists,
            cmd::get_runtime_logs,
            cmd::invoke_uwp_tool,
            cmd::copy_clash_env,
            cmd::get_proxies,
            cmd::force_refresh_proxies,
            cmd::get_providers_proxies,
            cmd::save_dns_config,
            cmd::apply_dns_config,
            cmd::check_dns_config_exists,
            cmd::get_dns_config_content,
            cmd::validate_dns_config,
            // verge
            cmd::get_verge_config,
            cmd::patch_verge_config,
            cmd::test_delay,
            cmd::get_app_dir,
            cmd::copy_icon_file,
            cmd::download_icon_cache,
            cmd::open_devtools,
            cmd::exit_app,
            cmd::get_network_interfaces_info,
            // profile
            cmd::get_profiles,
            cmd::enhance_profiles,
            cmd::patch_profiles_config,
            cmd::view_profile,
            cmd::patch_profile,
            cmd::create_profile,
            cmd::import_profile,
            cmd::reorder_profile,
            cmd::update_profile,
            cmd::delete_profile,
            cmd::read_profile_file,
            cmd::save_profile_file,
            cmd::get_next_update_time,
            cmd::update_profiles_on_startup,
            cmd::create_profile_from_share_link,
            // script validation
            cmd::script_validate_notice,
            cmd::validate_script_file,
            // clash api
            cmd::clash_api_get_proxy_delay,
            // backup
            cmd::create_webdav_backup,
            cmd::save_webdav_config,
            cmd::list_webdav_backup,
            cmd::delete_webdav_backup,
            cmd::restore_webdav_backup,
            // export diagnostic info for issue reporting
            cmd::export_diagnostic_info,
            // get system info for display
            cmd::get_system_info,
            // media unlock checker
            cmd::get_unlock_items,
            cmd::check_media_unlock,
            // light-weight model
            cmd::entry_lightweight_mode,
        ]);

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }

    // Macos Application Menu
    #[cfg(target_os = "macos")]
    {
        // Temporary Achived due to cannot CMD+C/V/A
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app_handle, e| match e {
        tauri::RunEvent::Ready | tauri::RunEvent::Resumed => {
            logging!(info, Type::System, true, "App ready or resumed");
            AppHandleManager::global().init(app_handle.clone());
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = AppHandleManager::global()
                    .get_handle()
                    .get_webview_window("main")
                {
                    logging!(info, Type::Window, true, "Setting macOS window title");
                    let _ = window.set_title("Koala Clash");
                }
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                AppHandleManager::global().set_activation_policy_regular();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            AppHandleManager::global().init(app_handle.clone());
        }
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        tauri::RunEvent::Exit => {
            let handle = core::handle::Handle::global();

            if handle.is_exiting() {
                logging!(
                    debug,
                    Type::System,
                    "Exit event triggered, but exit flow already executed, skip duplicate cleanup"
                );
            } else {
                logging!(debug, Type::System, "Exit event triggered, executing cleanup flow");
                handle.set_is_exiting();
                EventDrivenProxyManager::global().notify_app_stopping();
                feat::clean();
            }
        }
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            if label == "main" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        #[cfg(target_os = "macos")]
                        AppHandleManager::global().set_activation_policy_accessory();
                        if core::handle::Handle::global().is_exiting() {
                            return;
                        }
                        log::info!(target: "app", "closing window...");
                        api.prevent_close();
                        if let Some(window) = core::handle::Handle::global().get_window() {
                            let _ = window.hide();
                        } else {
                            logging!(
                                warn,
                                Type::Window,
                                true,
                                "Tried to hide window but it does not exist"
                            );
                        }
                    }
                    tauri::WindowEvent::Focused(true) => {
                        #[cfg(target_os = "macos")]
                        {
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().register("CMD+Q", "quit")
                            );
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().register("CMD+W", "hide")
                            );
                        }
                        {
                            let is_enable_global_hotkey = Config::verge()
                                .latest()
                                .enable_global_hotkey
                                .unwrap_or(true);
                            if !is_enable_global_hotkey {
                                logging_error!(Type::Hotkey, true, hotkey::Hotkey::global().init())
                            }
                        }
                    }
                    tauri::WindowEvent::Focused(false) => {
                        #[cfg(target_os = "macos")]
                        {
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().unregister("CMD+Q")
                            );
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().unregister("CMD+W")
                            );
                        }
                        {
                            let is_enable_global_hotkey = Config::verge()
                                .latest()
                                .enable_global_hotkey
                                .unwrap_or(true);
                            if !is_enable_global_hotkey {
                                logging_error!(Type::Hotkey, true, hotkey::Hotkey::global().reset())
                            }
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        #[cfg(target_os = "macos")]
                        {
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().unregister("CMD+Q")
                            );
                            logging_error!(
                                Type::Hotkey,
                                true,
                                hotkey::Hotkey::global().unregister("CMD+W")
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    });
}
