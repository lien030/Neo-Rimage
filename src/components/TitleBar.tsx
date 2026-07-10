import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Minus, X, Pin, PinOff, Languages } from "lucide-react";
import { LanguageType } from "@/lib/type";
import { useEffect, useState } from "react";
import { Button } from "./ui/button";
import { getCurrentWindow } from "@tauri-apps/api/window";
import i18n from "@/i18n/config";

export default function TitleBar() {
  const [isPinned, setIsPinned] = useState<boolean>(false);

  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(isPinned);
  }, [isPinned]);

  async function handleWindowClosed() {
    await getCurrentWindow().close();
  }

  async function handleWindowMinimize() {
    await getCurrentWindow().minimize();
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
      data-tauri-drag-region
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
          onClick={handleWindowClosed}
        >
          <X size={18} color="#888888" />
        </Button>
      </div>
    </header>
  );
}
