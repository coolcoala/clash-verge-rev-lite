import dayjs from "dayjs";
import i18next from "i18next";
import relativeTime from "dayjs/plugin/relativeTime";
import { SWRConfig, mutate } from "swr";
import { useEffect, useCallback, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useRoutes, useNavigate } from "react-router-dom";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { routers } from "./_routers";
import { getAxios } from "@/services/api";
import { useVerge } from "@/hooks/use-verge";
import { useThemeMode, useEnableLog } from "@/services/states";
import { useCustomTheme } from "@/components/layout/use-custom-theme";
import getSystem from "@/utils/get-system";
import "dayjs/locale/ru";
import "dayjs/locale/zh-cn";
import React from "react";
import { useListen } from "@/hooks/use-listen";
import { listen } from "@tauri-apps/api/event";
import { useClashInfo } from "@/hooks/use-clash";
import { initGlobalLogService } from "@/services/global-log-service";
import { invoke } from "@tauri-apps/api/core";
import { showNotice } from "@/services/noticeService";
import { Toaster } from "@/components/ui/sonner";
import { SidebarProvider, useSidebar } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/layout/sidebar";
import { useZoomControls } from "@/hooks/useZoomControls";
import { HwidErrorDialog } from "@/components/profile/hwid-error-dialog";
import { BaseLoadingOverlay } from "@/components/base";

const appWindow = getCurrentWebviewWindow();
export let portableFlag = false;

dayjs.extend(relativeTime);

const OS = getSystem();

// Notification Handler
const handleNoticeMessage = (
  status: string,
  msg: string,
  t: (key: string) => string,
  navigate: (path: string, options?: any) => void,
) => {
  console.log("[Notification Listener V2] Receiving a message:", status, msg);

  switch (status) {
    case "import_sub_url::ok":
      mutate("getProfiles").then(_ => {});
      navigate("/");
      showNotice("success", t("Import Subscription Successful"));
      sessionStorage.setItem("activateProfile", msg);
      break;
    case "import_sub_url::error":
      console.log(msg);
      // TODO: refactor please
      if (
        msg.toLowerCase().includes("device") ||
        msg.toLowerCase().includes("устройств")
      ) {
        window.dispatchEvent(
          new CustomEvent("show-hwid-error", { detail: msg }),
        );
      } else {
        showNotice("error", msg);
      }
      break;
    case "set_config::error":
      showNotice("error", msg);
      break;
    case "update_with_clash_proxy":
      showNotice(
        "success",
        `${t("Update with Clash proxy successfully")} ${msg}`,
      );
      break;
    case "update_retry_with_clash":
      showNotice("info", t("Update failed, retrying with Clash proxy..."));
      break;
    case "update_failed_even_with_clash":
      showNotice(
        "error",
        `${t("Update failed even with Clash proxy")}: ${msg}`,
      );
      break;
    case "update_failed":
      showNotice("error", msg);
      break;
    case "config_validate::boot_error":
      showNotice("error", `${t("Boot Config Validation Failed")} ${msg}`);
      break;
    case "config_validate::core_change":
      showNotice(
        "error",
        `${t("Core Change Config Validation Failed")} ${msg}`,
      );
      break;
    case "config_validate::error":
      showNotice("error", `${t("Config Validation Failed")} ${msg}`);
      break;
    case "config_validate::process_terminated":
      showNotice("error", t("Config Validation Process Terminated"));
      break;
    case "config_validate::stdout_error":
      showNotice("error", `${t("Config Validation Failed")} ${msg}`);
      break;
    case "config_validate::script_error":
      showNotice("error", `${t("Script File Error")} ${msg}`);
      break;
    case "config_validate::script_syntax_error":
      showNotice("error", `${t("Script Syntax Error")} ${msg}`);
      break;
    case "config_validate::script_missing_main":
      showNotice("error", `${t("Script Missing Main")} ${msg}`);
      break;
    case "config_validate::file_not_found":
      showNotice("error", `${t("File Not Found")} ${msg}`);
      break;
    case "config_validate::yaml_syntax_error":
      showNotice("error", `${t("YAML Syntax Error")} ${msg}`);
      break;
    case "config_validate::yaml_read_error":
      showNotice("error", `${t("YAML Read Error")} ${msg}`);
      break;
    case "config_validate::yaml_mapping_error":
      showNotice("error", `${t("YAML Mapping Error")} ${msg}`);
      break;
    case "config_validate::yaml_key_error":
      showNotice("error", `${t("YAML Key Error")} ${msg}`);
      break;
    case "config_validate::yaml_error":
      showNotice("error", `${t("YAML Error")} ${msg}`);
      break;
    case "config_validate::merge_syntax_error":
      showNotice("error", `${t("Merge File Syntax Error")} ${msg}`);
      break;
    case "config_validate::merge_mapping_error":
      showNotice("error", `${t("Merge File Mapping Error")} ${msg}`);
      break;
    case "config_validate::merge_key_error":
      showNotice("error", `${t("Merge File Key Error")} ${msg}`);
      break;
    case "config_validate::merge_error":
      showNotice("error", `${t("Merge File Error")} ${msg}`);
      break;
    case "config_core::change_success":
      showNotice("success", `${t("Core Changed Successfully")}: ${msg}`);
      break;
    case "config_core::change_error":
      showNotice("error", `${t("Failed to Change Core")}: ${msg}`);
      break;
    default: // Optional: Log unhandled statuses
      console.warn(`[Notification Listener V2] Unprocessed state: ${status}`);
      break;
  }
};

const Layout = () => {
  const mode = useThemeMode();
  useZoomControls();
  const isDark = mode !== "light";
  const { t } = useTranslation();
  useCustomTheme();
  const { verge } = useVerge();
  const { clashInfo } = useClashInfo();
  const [enableLog] = useEnableLog();
  const { language, start_page } = verge ?? {};
  const navigate = useNavigate();
  const location = useLocation();
  const routersEles = useRoutes(routers);
  const { addListener, setupCloseListener } = useListen();
  const initRef = useRef(false);

  // TODO: вот тут если что реализация спина на реакте, можно её использовать в другом туду..
  const [updatingCount, setUpdatingCount] = useState(0);

  const handleNotice = useCallback(
    (payload: [string, string]) => {
      const [status, msg] = payload;
      setTimeout(() => {
        try {
          handleNoticeMessage(status, msg, t, navigate);
        } catch (error) {
          console.error(
            "[Layout] Failure to process a notification message:",
            error,
          );
        }
      }, 0);
    },
    [t, navigate],
  );

  // Initialize the global logging service
  useEffect(() => {
    if (clashInfo) {
      const { server = "", secret = "" } = clashInfo;
      initGlobalLogService(server, secret, enableLog, "info");
    }
  }, [clashInfo, enableLog]);

  // Setting up a listener
  useEffect(() => {
    const listeners = [
      addListener("koala://refresh-clash-config", async () => {
        await getAxios(true);
        await mutate("getProxies");
        await mutate("getVersion");
        await mutate("getClashConfig");
        await mutate("getProxyProviders");
      }),

      addListener("koala://refresh-verge-config", () => {
        mutate("getVergeConfig").then(_ => {});
        mutate("getSystemProxy").then(_ => {});
        mutate("getAutotemProxy").then(_ => {});
      }),

      addListener("koala://notice-message", ({ payload }) =>
        handleNotice(payload as [string, string]),
      ),
      // Loader: profile update start/end
      addListener("profile-update-started", () => {
        setUpdatingCount((n) => n + 1);
      }),
      addListener("profile-update-completed", () => {
        setUpdatingCount((n) => Math.max(0, n - 1));
      }),
    ];

    const setupWindowListeners = async () => {
      const [hideUnlisten, showUnlisten] = await Promise.all([
        listen("koala://hide-window", () => appWindow.hide()),
        listen("koala://show-window", () => appWindow.show()),
      ]);

      return () => {
        hideUnlisten();
        showUnlisten();
      };
    };

    setupCloseListener().then(_ => {});
    const cleanupWindow = setupWindowListeners();

    return () => {
      setTimeout(() => {
        listeners.forEach((listener) => {
          if (typeof listener.then === "function") {
            listener
              .then((unlisten) => {
                try {
                  unlisten();
                } catch (error) {
                  console.error(
                    "[Layout] Failed to clear event listener:",
                    error,
                  );
                }
              })
              .catch((error) => {
                console.error(
                  "[Layout] Failed to get unlisten function:",
                  error,
                );
              });
          }
        });

        cleanupWindow
          .then((cleanup) => {
            try {
              cleanup();
            } catch (error) {
              console.error("[Layout] Failed to clear window listener:", error);
            }
          })
          .catch((error) => {
            console.error("[Layout] Failed to get cleanup function:", error);
          });
      }, 0);
    };
  }, [handleNotice]);

  useEffect(() => {
    if (initRef.current) {
      console.log(
        "[Layout] Initialization code has already been executed, skip",
      );
      return;
    }
    console.log("[Layout] Begin executing initialization code");
    initRef.current = true;

    let isInitialized = false;
    let initializationAttempts = 0;
    const maxAttempts = 3;

    const notifyBackend = async (action: string, stage?: string) => {
      try {
        if (stage) {
          console.log(`[Layout] Notification Backend ${action}: ${stage}`);
          await invoke("update_ui_stage", { stage });
        } else {
          console.log(`[Layout] Notification Backend ${action}`);
          await invoke("notify_ui_ready");
        }
      } catch (err) {
        console.error(`[Layout] Notification failure ${action}:`, err);
      }
    };

    const removeLoadingOverlay = () => {
      const initialOverlay = document.getElementById("initial-loading-overlay");
      if (initialOverlay) {
        console.log("[Layout] Remove loading indicator");
        initialOverlay.style.opacity = "0";
        setTimeout(() => {
          try {
            initialOverlay.remove();
          } catch (e) {
            console.log("[Layout] Load indicator has been removed");
          }
        }, 300);
      }
    };

    const performInitialization = async () => {
      if (isInitialized) {
        console.log("[Layout] Already initialized, skip");
        return;
      }

      initializationAttempts++;
      console.log(
        `[Layout] Start ${initializationAttempts} for the first time`,
      );

      try {
        removeLoadingOverlay();

        await notifyBackend("Loading phase", "Loading");

        await new Promise<void>((resolve) => {
          const checkReactMount = () => {
            const rootElement = document.getElementById("root");
            if (rootElement && rootElement.children.length > 0) {
              console.log("[Layout] React components are mounted");
              resolve();
            } else {
              setTimeout(checkReactMount, 50);
            }
          };

          checkReactMount();

          setTimeout(() => {
            console.log(
              "[Layout] React components mount check timeout, continue execution",
            );
            resolve();
          }, 2000);
        });

        await notifyBackend("DOM ready", "DomReady");

        await new Promise<void>((resolve) => {
          requestAnimationFrame(() => resolve());
        });

        await notifyBackend("Resource loading completed", "ResourcesLoaded");

        await notifyBackend("UI ready");

        isInitialized = true;
        console.log(
          `[Layout] The ${initializationAttempts} initialization is complete`,
        );
      } catch (error) {
        console.error(
          `[Layout] Initialization failure at ${initializationAttempts}:`,
          error,
        );

        if (initializationAttempts < maxAttempts) {
          console.log(
            `[Layout] The first ${initializationAttempts + 1} retry will be made after 500ms`,
          );
          setTimeout(performInitialization, 500);
        } else {
          console.error(
            "[Layout] All initialization attempts fail, perform emergency initialization",
          );

          removeLoadingOverlay();
          try {
            await notifyBackend("UI ready");
            isInitialized = true;
          } catch (e) {
            console.error("[Layout] Emergency initialization also failed:", e);
          }
        }
      }
    };

    let hasEventTriggered = false;

    const setupEventListener = async () => {
      try {
        console.log("[Layout] Start listening for startup completion events");
        return await listen("koala://startup-completed", () => {
          if (!hasEventTriggered) {
            console.log(
              "[Layout] Receive startup completion event, start initialization",
            );
            hasEventTriggered = true;
            performInitialization();
          }
        });
      } catch (err) {
        console.error(
          "[Layout] Failed to listen for startup completion event:",
          err,
        );
        return () => {};
      }
    };

    const checkImmediateInitialization = async () => {
      try {
        console.log("[Layout] Check if the backend is ready");
        await invoke("update_ui_stage", { stage: "Loading" });

        if (!hasEventTriggered && !isInitialized) {
          console.log(
            "[Layout] Backend is ready, start initialization immediately",
          );
          hasEventTriggered = true;
          await performInitialization();
        }
      } catch (err) {
        console.log(
          "[Layout] Backend not yet ready, waiting for startup completion event",
        );
      }
    };

    const backupInitialization = setTimeout(() => {
      if (!hasEventTriggered && !isInitialized) {
        console.warn(
          "[Layout] Standby initialization trigger: initialization not started within 1.5 seconds",
        );
        hasEventTriggered = true;
        performInitialization().then(_ => {});
      }
    }, 1500);

    const emergencyInitialization = setTimeout(() => {
      if (!isInitialized) {
        console.error(
          "[Layout] Emergency initialization trigger: initialization not completed within 5 seconds",
        );
        removeLoadingOverlay();
        notifyBackend("UI ready").catch(() => {});
        isInitialized = true;
      }
    }, 5000);

    const unlistenPromise = setupEventListener();

    setTimeout(checkImmediateInitialization, 100);

    return () => {
      clearTimeout(backupInitialization);
      clearTimeout(emergencyInitialization);
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Language and start page settings
  useEffect(() => {
    if (language) {
      dayjs.locale(language === "ru" ? "ru-ru" : language);
      i18next.changeLanguage(language).then(_ => {});
    }
  }, [language]);

  useEffect(() => {
    if (start_page) {
      navigate(start_page, { replace: true });
    }
  }, [start_page]);

  if (!routersEles) {
    return <div className="h-screen w-screen bg-background" />;
  }

  const AppLayout = () => {
    const { state, isMobile } = useSidebar();
    const location = useLocation();
    const routersEles = useRoutes(routers);

    return (
      <>
        <AppSidebar />
        <main className="h-screen w-full overflow-y-auto transition-[margin] duration-200 ease-linear">
          <div className="h-full w-full relative">
            {routersEles &&
              React.cloneElement(routersEles, { key: location.pathname })}
          </div>
          <BaseLoadingOverlay isLoading={updatingCount > 0} className="fixed" />
        </main>
        <HwidErrorDialog />
      </>
    );
  };

  return (
    <SWRConfig value={{ errorRetryCount: 3 }}>
      <SidebarProvider defaultOpen={false}>
        <AppLayout />
        <Toaster />
      </SidebarProvider>
    </SWRConfig>
  );
};

export default Layout;
