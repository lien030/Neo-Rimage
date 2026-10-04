import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import { createTaskDraft, resetCreateTaskDraft } from "@/features/create-task";
import i18n from "@/i18n/config";
import WebPTab from "./WebPTab";

afterEach(() => resetCreateTaskDraft());

describe("WebP quality controls", () => {
  it.each(["en", "zh", "ja"])("keeps quality editable in both modes in %s", async (language) => {
    await i18n.changeLanguage(language);
    for (const lossless of [false, true]) {
      createTaskDraft.webp.lossless = lossless;
      // An invalid value must remain editable so the user can correct it.
      createTaskDraft.webp.quality = "";
      const markup = renderToStaticMarkup(
        <TooltipProvider><WebPTab extensions={["webp"]} /></TooltipProvider>,
      );
      const label = i18n.t(lossless ? "webpCompressionEffort" : "quality");
      const input = (markup.match(/<input\b[^>]*>/g) ?? [])
        .find((element) => element.includes(`aria-label="${label}"`));
      expect(input).toBeDefined();
      expect(input).not.toContain(' disabled=""');
      expect(input).toContain('min="1"');
      expect(input).toContain('max="100"');
      expect(input).toContain('value=""');
      if (lossless) expect(markup).toContain(i18n.t("webpCompressionEffortDescription"));
    }
  });
});
