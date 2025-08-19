use crate::{
    cmd,
    config::{Config, PrfItem, PrfOption},
    core::{handle, CoreManager, *},
    logging,
    process::AsyncHandler,
    utils::logging::Type,
};
use anyhow::{bail, Result};

/// Toggle proxy profile
pub fn toggle_proxy_profile(profile_index: String) {
    AsyncHandler::spawn(|| async move {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        match cmd::patch_profiles_config_by_profile_index(app_handle, profile_index).await {
            Ok(_) => {
                let _ = tray::Tray::global().update_menu();
            }
            Err(err) => {
                log::error!(target: "app", "{err}");
            }
        }
    });
}

/// Internal: apply core update and handle logging/notifications
async fn apply_core_update() {
    logging!(
        info,
        Type::Config,
        true,
        "[Subscription Update] Update core configuration"
    );
    match CoreManager::global().update_config().await {
        Ok(_) => {
            logging!(
                info,
                Type::Config,
                true,
                "[Subscription Update] Update succeeded"
            );
            handle::Handle::refresh_clash();
        }
        Err(err) => {
            logging!(
                error,
                Type::Config,
                true,
                "[Subscription Update] Update failed: {}",
                err
            );
            handle::Handle::notice_message("update_failed", format!("{err}"));
            log::error!(target: "app", "{err}");
        }
    }
}

/// Internal: whether the given uid is the current active profile
fn is_current_profile(uid: &String) -> bool {
    let profiles = Config::profiles();
    let profiles = profiles.latest();
    Some(uid.to_owned()) == profiles.get_current()
}

/// Internal: download subscription and update profiles.
/// Returns whether we should update core config next.
async fn download_and_update(
    uid: &String,
    url: &str,
    merged_opt: Option<PrfOption>,
    auto_refresh: bool,
) -> Result<bool> {
    match PrfItem::from_url(url, None, None, merged_opt.clone()).await {
        Ok(item) => {
            log::info!(target: "app", "[Subscription Update] Subscription config updated successfully");
            let profiles = Config::profiles();
            let mut profiles = profiles.latest();
            profiles.update_item(uid.clone(), item)?;

            let is_current = is_current_profile(uid);
            log::info!(target: "app", "[Subscription Update] Is current active subscription: {is_current}");
            Ok(is_current && auto_refresh)
        }
        Err(err) => {
            log::warn!(target: "app", "[Subscription Update] Normal update failed: {err}, trying to update via Clash proxy");
            handle::Handle::notice_message("update_retry_with_clash", uid.clone());

            let original_with_proxy = merged_opt.as_ref().and_then(|o| o.with_proxy);
            let original_self_proxy = merged_opt.as_ref().and_then(|o| o.self_proxy);

            let mut fallback_opt = merged_opt.unwrap_or_default();
            fallback_opt.with_proxy = Some(false);
            fallback_opt.self_proxy = Some(true);

            match PrfItem::from_url(url, None, None, Some(fallback_opt)).await {
                Ok(mut item) => {
                    log::info!(target: "app", "[Subscription Update] Update via Clash proxy succeeded");

                    if let Some(option) = item.option.as_mut() {
                        option.with_proxy = original_with_proxy;
                        option.self_proxy = original_self_proxy;
                    }

                    let profiles = Config::profiles();
                    let mut profiles = profiles.latest();
                    profiles.update_item(uid.clone(), item.clone())?;

                    let profile_name = item.name.clone().unwrap_or_else(|| uid.clone());
                    handle::Handle::notice_message("update_with_clash_proxy", profile_name);

                    let is_current = is_current_profile(uid);
                    log::info!(target: "app", "[Subscription Update] Is current active subscription: {is_current}");
                    Ok(is_current && auto_refresh)
                }
                Err(retry_err) => {
                    log::error!(target: "app", "[Subscription Update] Update via Clash proxy still failed: {retry_err}");
                    handle::Handle::notice_message(
                        "update_failed_even_with_clash",
                        format!("{retry_err}"),
                    );
                    Err(retry_err)
                }
            }
        }
    }
}

/// Update a profile
/// If updating current profile, activate it
/// auto_refresh: 是否自动更新配置和刷新前端
pub async fn update_profile(
    uid: String,
    option: Option<PrfOption>,
    auto_refresh: Option<bool>,
    skip_fetch: Option<bool>,
) -> Result<()> {
    logging!(
        info,
        Type::Config,
        true,
        "[Subscription Update] Start updating subscription {}",
        uid
    );
    let auto_refresh = auto_refresh.unwrap_or(true); // 默认为true，保持兼容性

    // 如果指定跳过拉取，仅进行核心配置更新
    if skip_fetch.unwrap_or(false) {
        if is_current_profile(&uid) && auto_refresh {
            apply_core_update().await;
        }
        return Ok(());
    }

    let url_opt = {
        let profiles = Config::profiles();
        let profiles = profiles.latest();
        let item = profiles.get_item(&uid)?;
        let is_remote = item.itype.as_ref().is_some_and(|s| s == "remote");

        if !is_remote {
            log::info!(target: "app", "[Subscription Update] {uid} is not a remote subscription, skipping update");
            None // 非远程订阅直接更新
        } else if item.url.is_none() {
            log::warn!(target: "app", "[Subscription Update] {uid} is missing URL, cannot update");
            bail!("failed to get the profile item url");
        } else {
            log::info!(target: "app",
                "[Subscription Update] {} is a remote subscription, URL: {}",
                uid,
                item.url.clone().unwrap()
            );
            Some((item.url.clone().unwrap(), item.option.clone()))
        }
    };

    let should_update = match url_opt {
        Some((url, opt)) => {
            log::info!(target: "app", "[Subscription Update] Start downloading new subscription content");
            let merged_opt = PrfOption::merge(opt, option);
            download_and_update(&uid, &url, merged_opt, auto_refresh).await?
        }
        None => auto_refresh,
    };

    if should_update {
        apply_core_update().await;
    }

    Ok(())
}

/// 增强配置
pub async fn enhance_profiles() -> Result<()> {
    crate::core::CoreManager::global()
        .update_config()
        .await
        .map(|_| ())
}
