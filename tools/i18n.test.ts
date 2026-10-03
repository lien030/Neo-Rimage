import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import * as ts from "typescript";
import { describe, expect, it } from "vitest";

import en from "../src/i18n/en.json";
import ja from "../src/i18n/ja.json";
import zh from "../src/i18n/zh.json";

function flattenTranslations(
  resource: Record<string, unknown>,
  prefix = "",
): Record<string, string> {
  return Object.fromEntries(Object.entries(resource).flatMap(([key, value]) => {
    const fullKey = prefix ? prefix + "." + key : key;
    if (typeof value === "string") {
      return [[fullKey, value]];
    }
    return Object.entries(flattenTranslations(value as Record<string, unknown>, fullKey));
  }));
}

function sourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const fullPath = join(directory, entry.name);
    return entry.isDirectory() ? sourceFiles(fullPath) : [fullPath];
  });
}

const resources = { en: flattenTranslations(en), zh: flattenTranslations(zh), ja: flattenTranslations(ja) };
const frontendRoot = fileURLToPath(new URL("../src/", import.meta.url));
const backendRoot = fileURLToPath(new URL("../src-tauri/src/", import.meta.url));

describe("translation coverage", () => {
  it.each(["zh", "ja"] as const)("keeps %s keys and interpolation placeholders in sync with English", (language) => {
    expect(Object.keys(resources[language]).sort()).toEqual(Object.keys(resources.en).sort());
    for (const [key, value] of Object.entries(resources.en)) {
      expect(resources[language][key].trim(), key).not.toBe("");
      const placeholders = (text: string) => [...text.matchAll(/\{\{([^{}]+)\}\}/g)]
        .map(match => match[1].trim()).sort();
      expect(placeholders(resources[language][key]), key).toEqual(placeholders(value));
    }
  });

  it("covers every backend error message key in all languages", () => {
    const keys = new Set<string>();
    for (const file of sourceFiles(backendRoot).filter(file => file.endsWith(".rs") && !file.endsWith("tests.rs"))) {
      for (const match of readFileSync(file, "utf8").matchAll(/"(errors\.[A-Za-z0-9_.]+)"/g)) {
        keys.add(match[1]);
      }
    }
    expect(keys.size).toBeGreaterThan(0);
    for (const translations of Object.values(resources)) {
      for (const key of keys) expect(translations[key], key).toBeTruthy();
    }
  });

  it("covers every job status, worker status, processing stage and resize filter", () => {
    const contractsPath = join(frontendRoot, "lib/ipc/contracts.ts");
    const contracts = ts.createSourceFile(
      contractsPath, readFileSync(contractsPath, "utf8"), ts.ScriptTarget.Latest, true,
    );
    const namespaces: Record<string, string> = {
      JobStatus: "jobStatus",
      WorkerSlotStatus: "workerStatus",
      ProcessingStage: "processingStage",
      ResizeFilter: "resizeFilters",
    };
    for (const [name, namespace] of Object.entries(namespaces)) {
      const declaration = contracts.statements.find(
        (node): node is ts.TypeAliasDeclaration => ts.isTypeAliasDeclaration(node) && node.name.text === name,
      );
      expect(declaration, name).toBeDefined();
      if (!declaration || !ts.isUnionTypeNode(declaration.type)) throw new Error("Expected enum union: " + name);
      for (const member of declaration.type.types) {
        if (!ts.isLiteralTypeNode(member) || !ts.isStringLiteral(member.literal)) throw new Error("Expected string enum: " + name);
        const key = namespace + "." + member.literal.text;
        for (const translations of Object.values(resources)) expect(translations[key], key).toBeTruthy();
      }
    }
  });

  it("resolves static translation calls and rejects hardcoded interface prose", () => {
    const allowedText = new Set(["px", "RGB", "YCbCr", "Neo Rimage"]);
    const untranslated: string[] = [];
    const missingKeys: string[] = [];
    for (const file of sourceFiles(frontendRoot).filter(file => /\.tsx?$/.test(file) && !/\.test\.tsx?$/.test(file))) {
      const source = ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true);
      function visit(node: ts.Node) {
        const position = source.getLineAndCharacterOfPosition(node.getStart());
        const location = file + ":" + (position.line + 1);
        if (ts.isJsxText(node)) {
          const text = node.text.trim();
          if (/[A-Za-z\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/u.test(text) && !allowedText.has(text)) {
            untranslated.push(location + " " + text);
          }
        }
        if (ts.isJsxAttribute(node) && /^(title|placeholder|aria-label|alt)$/.test(node.name.getText(source))
          && node.initializer && ts.isStringLiteral(node.initializer) && !allowedText.has(node.initializer.text)) {
          untranslated.push(location + " " + node.initializer.text);
        }
        if (ts.isCallExpression(node) && node.arguments[0] && ts.isStringLiteral(node.arguments[0])) {
          const expression = node.expression;
          const isTranslation = ts.isIdentifier(expression) && ["t", "translate"].includes(expression.text)
            || ts.isPropertyAccessExpression(expression) && expression.name.text === "t";
          if (isTranslation) {
            const key = node.arguments[0].text;
            for (const translations of Object.values(resources)) {
              if (!(key in translations) && !(key + "_other" in translations)) missingKeys.push(location + " " + key);
            }
          }
          if (ts.isPropertyAccessExpression(expression) && expression.expression.getText(source) === "toast") {
            untranslated.push(location + " " + node.arguments[0].text);
          }
        }
        ts.forEachChild(node, visit);
      }
      visit(source);
    }
    expect(untranslated).toEqual([]);
    expect(missingKeys).toEqual([]);
  });
});
