import { useEffect, useRef, useState, useCallback } from "react";
import { useLockFn } from "ahooks";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { emit } from "@tauri-apps/api/event";
import { nanoid } from "nanoid";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  arrayMove,
} from "@dnd-kit/sortable";

// Новые импорты
import { useVerge } from "@/hooks/use-verge";
import { TestViewer, TestViewerRef } from "@/components/test/test-viewer";
import { TestItem } from "@/components/test/test-item";
import { ScrollTopButton } from "@/components/layout/scroll-top-button";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Menu, Play, Plus } from "lucide-react";
import { cn } from "@root/lib/utils";

// Иконки тестов
import apple from "@/assets/image/test/apple.svg?raw";
import github from "@/assets/image/test/github.svg?raw";
import google from "@/assets/image/test/google.svg?raw";
import youtube from "@/assets/image/test/youtube.svg?raw";

const TestPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  const { verge, mutateVerge, patchVerge } = useVerge();

  // Логика для "липкой" шапки и скролла
  const scrollerRef = useRef<HTMLDivElement>(null);
  const [isScrolled, setIsScrolled] = useState(false);
  const [showScrollTop, setShowScrollTop] = useState(false);

  useEffect(() => {
    const currentScroller = scrollerRef.current;
    if (!currentScroller) return;

    const handleScroll = () => {
      const scrollTop = currentScroller.scrollTop;
      setIsScrolled(scrollTop > 5);
      setShowScrollTop(scrollTop > 100);
    };

    currentScroller.addEventListener("scroll", handleScroll);
    return () => currentScroller.removeEventListener("scroll", handleScroll);
  }, []);

  const scrollToTop = useCallback(() => {
    scrollerRef.current?.scrollTo({
      top: 0,
      behavior: "smooth",
    });
  }, []);

  // Список тестов
  const testList = verge?.test_list ?? [
    { uid: nanoid(), name: "Apple", url: "https://www.apple.com", icon: apple },
    {
      uid: nanoid(),
      name: "GitHub",
      url: "https://www.github.com",
      icon: github,
    },
    {
      uid: nanoid(),
      name: "Google",
      url: "https://www.google.com",
      icon: google,
    },
    {
      uid: nanoid(),
      name: "Youtube",
      url: "https://www.youtube.com",
      icon: youtube,
    },
  ];

  const onTestListItemChange = (
    uid: string,
    patch?: Partial<IVergeTestItem>,
  ) => {
    if (patch) {
      const newList = testList.map((x) =>
        x.uid === uid ? { ...x, ...patch } : x,
      );
      mutateVerge({ ...verge, test_list: newList }, false).then(r => {});
    } else {
      mutateVerge().then(r => {});
    }
  };

  const onDeleteTestListItem = (uid: string) => {
    const newList = testList.filter((x) => x.uid !== uid);
    patchVerge({ test_list: newList }).then(r => {});
    mutateVerge({ ...verge, test_list: newList }, false).then(r => {});
  };

  const onDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      const oldIndex = testList.findIndex((x) => x.uid === active.id);
      const newIndex = testList.findIndex((x) => x.uid === over.id);
      if (oldIndex === -1 || newIndex === -1) return;
      const newList = arrayMove(testList, oldIndex, newIndex);
      await mutateVerge({ ...verge, test_list: newList }, false);
      await patchVerge({ test_list: newList });
    }
  };

  useEffect(() => {
    if (verge && !verge.test_list) {
      patchVerge({ test_list: testList }).then(r => {});
    }
  }, [verge, patchVerge, testList]);

  const viewerRef = useRef<TestViewerRef>(null);

  const menuItems = [
    { label: t("Home"), path: "/home" },
    { label: t("Profiles"), path: "/profile" },
    { label: t("Settings"), path: "/settings" },
    { label: t("Logs"), path: "/logs" },
    { label: t("Proxies"), path: "/proxies" },
    { label: t("Connections"), path: "/connections" },
    { label: t("Rules"), path: "/rules" },
  ];

  return (
    <div className="h-full w-full relative">
      <div
        className={cn(
          "absolute top-0 left-0 right-0 z-10 p-4 transition-all duration-200",
          { "bg-background/80 backdrop-blur-sm shadow-sm": isScrolled },
        )}
      >
        <div className="flex justify-between items-center">
          <h2 className="text-2xl font-semibold tracking-tight">{t("Test")}</h2>
          <div className="flex items-center gap-2">
            <Button size="sm" onClick={() => emit("koala://test-all")}>
              <Play className="mr-2 h-4 w-4" />
              {t("Test All")}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => viewerRef.current?.create()}
            >
              <Plus className="mr-2 h-4 w-4" />
              {t("New")}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" title={t("Menu")}>
                  <Menu className="h-5 w-5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuLabel>{t("Menu")}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                {menuItems.map((item) => (
                  <DropdownMenuItem
                    key={item.path}
                    onSelect={() => navigate(item.path)}
                    disabled={location.pathname === item.path}
                  >
                    {item.label}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </div>

      <div
        ref={scrollerRef}
        className="absolute top-0 left-0 right-0 bottom-0 pt-20 overflow-y-auto"
      >
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={onDragEnd}
        >
          <div className="p-4">
            <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7 gap-4">
              <SortableContext items={testList.map((x) => x.uid)}>
                {testList.map((item) => (
                  <TestItem
                    key={item.uid}
                    id={item.uid}
                    itemData={item}
                    onEdit={() => viewerRef.current?.edit(item)}
                    onDelete={onDeleteTestListItem}
                  />
                ))}
              </SortableContext>
            </div>
          </div>
        </DndContext>
      </div>

      <ScrollTopButton onClick={scrollToTop} show={showScrollTop} />
      <TestViewer ref={viewerRef} onChange={onTestListItemChange} />
    </div>
  );
};

export default TestPage;
