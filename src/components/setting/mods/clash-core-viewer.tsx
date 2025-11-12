import { forwardRef, useImperativeHandle, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLockFn } from "ahooks";
import { mutate } from "swr";

// Новые импорты
import { DialogRef } from "@/components/base";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Loader2, Replace, RotateCw } from "lucide-react";
import { cn } from "@root/lib/utils";

// Логика и сервисы
import { useVerge } from "@/hooks/use-verge";
import { changeClashCore, restartCore } from "@/services/cmds";
import { closeAllConnections, upgradeCore } from "@/services/api";
import { showNotice } from "@/services/noticeService";

// Константы и интерфейсы
const VALID_CORE = [
  { name: "Mihomo", core: "koala-mihomo", chip: "Release Version" },
  { name: "Mihomo Alpha", core: "koala-mihomo-alpha", chip: "Alpha Version" },
  { name: "Mihomo Alpha Smart", core: "koala-mihomo-alpha-smart", chip: "Alpha Version" },
];

export const ClashCoreViewer = forwardRef<DialogRef>((props, ref) => {
  const { t } = useTranslation();
  const { verge, mutateVerge } = useVerge();

  const [open, setOpen] = useState(false);
  const [upgrading, setUpgrading] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [changingCore, setChangingCore] = useState<string | null>(null);

  useImperativeHandle(ref, () => ({
    open: () => setOpen(true),
    close: () => setOpen(false),
  }));

  const { clash_core = "koala-mihomo" } = verge ?? {};

  const onCoreChange = useLockFn(async (core: string) => {
    if (core === clash_core) return;
    try {
      setChangingCore(core);
      closeAllConnections();
      const errorMsg = await changeClashCore(core);
      if (errorMsg) {
        showNotice("error", errorMsg);
        setChangingCore(null);
        return;
      }
      mutateVerge();
      setTimeout(() => {
        mutate("getClashConfig");
        mutate("getVersion");
        setChangingCore(null);
      }, 500);
    } catch (err: any) {
      setChangingCore(null);
      showNotice("error", err.message || err.toString());
    }
  });

  const onRestart = useLockFn(async () => {
    try {
      setRestarting(true);
      await restartCore();
      showNotice("success", t(`Clash Core Restarted`));
    } catch (err: any) {
      showNotice("error", err.message || err.toString());
    } finally {
      setRestarting(false);
    }
  });

  const onUpgrade = useLockFn(async () => {
    try {
      setUpgrading(true);
      await upgradeCore();
      showNotice("success", t(`Core Version Updated`));
    } catch (err: any) {
      const errMsg = err.response?.data?.message || err.toString();
      const showMsg = errMsg.includes("already using latest version")
        ? "Already Using Latest Core Version"
        : errMsg;
      showNotice("error", t(showMsg));
    } finally {
      setUpgrading(false);
    }
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-md">
        {/* --- НАЧАЛО ИЗМЕНЕНИЙ --- */}
        {/* Добавляем отступ справа (pr-12), чтобы освободить место для крестика */}
        <DialogHeader className="pr-12">
          <div className="flex justify-between items-center">
            <DialogTitle>{t("Clash Core")}</DialogTitle>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                disabled={restarting || changingCore !== null}
                onClick={onUpgrade}
              >
                {upgrading ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Replace className="mr-2 h-4 w-4" />
                )}
                {t("Upgrade")}
              </Button>
              <Button
                size="sm"
                disabled={upgrading || changingCore !== null}
                onClick={onRestart}
              >
                {restarting ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <RotateCw className="mr-2 h-4 w-4" />
                )}
                {t("Restart")}
              </Button>
            </div>
          </div>
        </DialogHeader>
        {/* --- КОНЕЦ ИЗМЕНЕНИЙ --- */}

        <div className="space-y-2 py-4">
          {VALID_CORE.map((each) => {
            const isSelected = each.core === clash_core;
            const isChanging = changingCore === each.core;
            const isDisabled = changingCore !== null || restarting || upgrading;

            return (
              <div
                key={each.core}
                data-selected={isSelected}
                onClick={() => !isDisabled && onCoreChange(each.core)}
                className={cn(
                  "flex items-center justify-between p-3 rounded-md transition-colors",
                  isDisabled
                    ? "cursor-not-allowed opacity-50"
                    : "cursor-pointer hover:bg-accent",
                  isSelected && "bg-accent",
                )}
              >
                <div>
                  <p className="font-semibold text-sm">{each.name}</p>
                  <p className="text-xs text-muted-foreground">{`/${each.core}`}</p>
                </div>
                <div className="w-28 text-right flex justify-end">
                  {isChanging ? (
                    <Loader2 className="h-5 w-5 animate-spin" />
                  ) : (
                    <Badge variant={isSelected ? "default" : "secondary"}>
                      {t(each.chip)}
                    </Badge>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="outline">
              {t("Close")}
            </Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
