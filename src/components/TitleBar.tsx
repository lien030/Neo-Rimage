import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Languages,
  Maximize2,
  Minimize2,
  Minus,
  Pin,
  PinOff,
  X,
} from "lucide-react";
import { LanguageType } from "@/lib/type";
import { useEffect, useState } from "react";
import { Button } from "./ui/button";
import { getCurrentWindow } from "@tauri-apps/api/window";
import i18n from "@/i18n/config";

export default function TitleBar() {
  const [isPinned, setIsPinned] = useState<boolean>(false);
  const [isMaximized, setIsMaximized] = useState<boolean>(false);

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

  async function handleWindowClosed() {
    await getCurrentWindow().close();
  }

  async function handleWindowMinimize() {
    await getCurrentWindow().minimize();
  }

  async function handleWindowMaximize() {
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  }

  function handleWindowPinned() {
    setIsPinned(!isPinned);
  }

  function updateLanguage(lang: LanguageType) {
    localStorage.setItem("language", lang);
    i18n.changeLanguage(lang);
  }

  return (
    <header
      data-tauri-drag-region="deep"
      className="absolute top-0 h-14 w-full -mt-[1px] px-4 flex justify-between items-center select-none"
    >
      <p className="font-semibold text-lg text-primary">neo-rimage</p>
      <div className="flex gap-4">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="hover:bg-zinc-200/50 h-8 w-8"
            >
              <Languages color="#888888" size={20} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            onCloseAutoFocus={(e) => e.preventDefault()}
          >
            <DropdownMenuItem
              onClick={() => {
                updateLanguage("zh");
              }}
            >
              简体中文
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                updateLanguage("ja");
              }}
            >
              日本語
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                updateLanguage("en");
              }}
            >
              English
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          variant="ghost"
          size="icon"
          className="hover:bg-zinc-200/50 h-8 w-8"
          onClick={handleWindowPinned}
        >
          {isPinned ? (
            <Pin size={18} color="#888888" />
          ) : (
            <PinOff size={18} color="#888888" />
          )}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="hover:bg-zinc-200/50 h-8 w-8"
          onClick={handleWindowMinimize}
        >
          <Minus size={18} color="#888888" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="hover:bg-zinc-200/50 h-8 w-8"
          onClick={handleWindowMaximize}
          aria-label={isMaximized ? "Restore window" : "Maximize window"}
          title={isMaximized ? "Restore" : "Maximize"}
        >
          {isMaximized ? (
            <Minimize2 size={18} color="#888888" />
          ) : (
            <Maximize2 size={18} color="#888888" />
          )}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="hover:bg-zinc-200/50 h-8 w-8"
          onClick={handleWindowClosed}
        >
          <X size={18} color="#888888" />
        </Button>
      </div>
    </header>
  );
}
