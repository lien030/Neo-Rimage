import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Copy as RestoreIcon,
  Languages,
  Maximize2,
  Minus,
  Pin,
  PinOff,
  X,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { Button } from "./ui/button";
import { getCurrentWindow } from "@tauri-apps/api/window";
import i18n, {
  LANGUAGE_STORAGE_KEY,
  type SupportedLanguage,
} from "@/i18n/config";
import { useTranslation } from "react-i18next";

const WINDOW_CONTROL_CLASS = "h-8 w-8 hover:bg-zinc-200/50";
const WINDOW_ICON_COLOR = "#888888";

const LANGUAGES: readonly { value: SupportedLanguage; label: string }[] = [
  { value: "zh", label: "简体中文" },
  { value: "ja", label: "日本語" },
  { value: "en", label: "English" },
];

export default function TitleBar() {
  const { t } = useTranslation();
  const [isPinned, setIsPinned] = useState<boolean>(false);
  const [isMaximized, setIsMaximized] = useState<boolean>(false);

  const languageLabel = t("changeLanguage");
  const pinLabel = t(isPinned ? "unpinWindow" : "pinWindow");
  const minimizeLabel = t("minimizeWindow");
  const maximizeLabel = t(isMaximized ? "restoreWindow" : "maximizeWindow");
  const closeLabel = t("closeWindow");

  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(isPinned);
  }, [isPinned]);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let isMounted = true;
    let unlisten: (() => void) | undefined;

    async function syncMaximizedState() {
      const maximized = await appWindow.isMaximized();

      if (isMounted) {
        setIsMaximized(maximized);
      }
    }

    void syncMaximizedState();
    void appWindow
      .onResized(() => {
        void syncMaximizedState();
      })
      .then((unlistenWindowResized) => {
        if (isMounted) {
          unlisten = unlistenWindowResized;
        } else {
          unlistenWindowResized();
        }
      });

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);

  async function handleClose() {
    await getCurrentWindow().close();
  }

  async function handleMinimize() {
    await getCurrentWindow().minimize();
  }

  async function handleToggleMaximize() {
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  }

  function handleTogglePinned() {
    setIsPinned((currentlyPinned) => !currentlyPinned);
  }

  function updateLanguage(lang: SupportedLanguage) {
    localStorage.setItem(LANGUAGE_STORAGE_KEY, lang);
    void i18n.changeLanguage(lang);
  }

  return (
    <header
      data-tauri-drag-region="deep"
      className="absolute top-0 -mt-[1px] flex h-14 w-full select-none items-center justify-between px-4"
    >
      <p className="text-lg font-semibold text-primary">neo-rimage</p>
      <div className="flex gap-4">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className={WINDOW_CONTROL_CLASS}
              aria-label={languageLabel}
              title={languageLabel}
            >
              <Languages color={WINDOW_ICON_COLOR} size={20} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            onCloseAutoFocus={(e) => e.preventDefault()}
          >
            {LANGUAGES.map((language) => (
              <DropdownMenuItem
                key={language.value}
                onClick={() => updateLanguage(language.value)}
              >
                {language.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <WindowControlButton
          label={pinLabel}
          onClick={handleTogglePinned}
        >
          {isPinned ? (
            <Pin size={18} color={WINDOW_ICON_COLOR} />
          ) : (
            <PinOff size={18} color={WINDOW_ICON_COLOR} />
          )}
        </WindowControlButton>
        <WindowControlButton
          label={minimizeLabel}
          onClick={handleMinimize}
        >
          <Minus size={18} color={WINDOW_ICON_COLOR} />
        </WindowControlButton>
        <WindowControlButton
          label={maximizeLabel}
          onClick={handleToggleMaximize}
        >
          {isMaximized ? (
            <RestoreIcon size={17} color={WINDOW_ICON_COLOR} />
          ) : (
            <Maximize2 size={18} color={WINDOW_ICON_COLOR} />
          )}
        </WindowControlButton>
        <WindowControlButton label={closeLabel} onClick={handleClose}>
          <X size={18} color={WINDOW_ICON_COLOR} />
        </WindowControlButton>
      </div>
    </header>
  );
}

function WindowControlButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void | Promise<void>;
  children: ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={WINDOW_CONTROL_CLASS}
      onClick={onClick}
      aria-label={label}
      title={label}
    >
      {children}
    </Button>
  );
}
