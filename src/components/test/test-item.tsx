import { useEffect, useState } from "react";
import { useLockFn } from "ahooks";
import { useTranslation } from "react-i18next";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { UnlistenFn } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";

import { useListen } from "@/hooks/use-listen";
import { showNotice } from "@/services/noticeService";
import delayManager from "@/services/delay";
import { cmdTestDelay, downloadIconCache } from "@/services/cmds";

// Новые импорты
import { BaseLoading } from "@/components/base";
import { TestBox } from "./test-box"; // Наш рефакторенный компонент
import { Separator } from "@/components/ui/separator";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Languages } from "lucide-react"; // Новая иконка

// Вспомогательная функция для цвета задержки
const getDelayColorClass = (delay: number): string => {
  if (delay < 0 || delay >= 10000) return "text-destructive";
  if (delay >= 500) return "text-destructive";
  if (delay >= 200) return "text-yellow-500";
  return "text-green-500";
};

interface Props {
  id: string;
  itemData: IVergeTestItem;
  onEdit: () => void;
  onDelete: (uid: string) => void;
}

export const TestItem = (props: Props) => {
  const { itemData, onEdit, onDelete: onDeleteItem } = props;
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: props.id });
  const { t } = useTranslation();

  const [delay, setDelay] = useState(-1);
  const { uid, name, icon, url } = itemData;
  const [iconCachePath, setIconCachePath] = useState("");
  const { addListener } = useListen();

  const onDelay = useLockFn(async () => {
    setDelay(-2); // Состояние загрузки
    const result = await cmdTestDelay(url);
    setDelay(result);
  });

  const getFileName = (url: string) => url.substring(url.lastIndexOf("/") + 1);

  async function initIconCachePath() {
    if (icon && icon.trim().startsWith("http")) {
      const fileName = uid + "-" + getFileName(icon);
      const iconPath = await downloadIconCache(icon, fileName);
      setIconCachePath(convertFileSrc(iconPath));
    }
  }

  useEffect(() => {
    initIconCachePath();
  }, [icon]);

  const onDelete = useLockFn(async () => {
    try {
      onDeleteItem(uid);
    } catch (err: any) {
      showNotice("error", err.message || err.toString());
    }
  });

  const menu = [
    { label: "Edit", handler: onEdit },
    { label: "Delete", handler: onDelete, isDestructive: true },
  ];

  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    const setupListener = async () => {
      if (unlistenFn) unlistenFn();
      unlistenFn = await addListener("koala://test-all", onDelay);
    };
    setupListener().then(_ => {});
    return () => {
      unlistenFn?.();
    };
  }, [url, addListener, onDelay]);

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 100 : undefined,
  };

  return (
    <div style={style} ref={setNodeRef} {...attributes}>
      <ContextMenu>
        <ContextMenuTrigger>
          <TestBox>
            {/* Мы применяем `listeners` к иконке, чтобы за нее можно было таскать */}
            <div
              {...listeners}
              className="flex h-12 cursor-move items-center justify-center"
            >
              {icon ? (
                <img
                  src={
                    icon.startsWith("data")
                      ? icon
                      : icon.startsWith("<svg")
                        ? `data:image/svg+xml;base64,${btoa(icon)}`
                        : iconCachePath || icon
                  }
                  className="h-10"
                  alt={name}
                />
              ) : (
                <Languages className="h-10 w-10 text-muted-foreground" />
              )}
            </div>

            <p
              className="mt-1 text-center text-sm font-semibold truncate"
              title={name}
            >
              {name}
            </p>

            <Separator className="my-2" />

            <div
              className="flex h-6 items-center justify-center text-sm font-medium"
              onClick={(e) => {
                e.stopPropagation();
                onDelay();
              }}
            >
              {delay === -2 ? (
                <BaseLoading className="h-4 w-4" />
              ) : delay === -1 ? (
                <span className="cursor-pointer rounded-md px-2 py-0.5 hover:bg-accent">
                  {t("Test")}
                </span>
              ) : (
                <span
                  className={`cursor-pointer rounded-md px-2 py-0.5 hover:bg-accent ${getDelayColorClass(delay)}`}
                >
                  {delayManager.formatDelay(delay)} ms
                </span>
              )}
            </div>
          </TestBox>
        </ContextMenuTrigger>

        <ContextMenuContent>
          {menu.map((item) => (
            <ContextMenuItem
              key={item.label}
              onClick={item.handler}
              className={item.isDestructive ? "text-destructive" : ""}
            >
              {t(item.label)}
            </ContextMenuItem>
          ))}
        </ContextMenuContent>
      </ContextMenu>
    </div>
  );
};
