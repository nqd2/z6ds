import type { Monaco } from "@monaco-editor/react";

export const MONACO_Z6DS_THEME = "z6ds-dark";

let registered = false;

export function registerMonacoZ6dsTheme(monaco: Monaco): void {
  if (registered) return;
  registered = true;

  monaco.editor.defineTheme(MONACO_Z6DS_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "7a7e85", fontStyle: "italic" },
      { token: "keyword", foreground: "cf8e6d" },
      { token: "keyword.control", foreground: "cf8e6d" },
      { token: "string", foreground: "6aab73" },
      { token: "number", foreground: "2aacb8" },
      { token: "type", foreground: "c77dbb" },
      { token: "type.identifier", foreground: "c77dbb" },
      { token: "identifier", foreground: "bcbec4" },
      { token: "delimiter", foreground: "bcbec4" },
      { token: "operator", foreground: "bcbec4" },
      { token: "function", foreground: "56a8f5" },
      { token: "variable", foreground: "bcbec4" },
      { token: "constant", foreground: "c77dbb" },
      { token: "tag", foreground: "cf8e6d" },
      { token: "attribute.name", foreground: "e8a33e" },
      { token: "attribute.value", foreground: "6aab73" },
      { token: "metatag", foreground: "7a7e85" },
      { token: "macro", foreground: "cf8e6d" },
      { token: "predefined", foreground: "56a8f5" },
    ],
    colors: {
      "editor.background": "#141518",
      "editor.foreground": "#c4c6cd",
      "editor.lineHighlightBackground": "#25262c",
      "editor.selectionBackground": "#373b39",
      "editor.inactiveSelectionBackground": "#373b3950",
      "editorCursor.foreground": "#c4c6cd",
      "editorLineNumber.foreground": "#4e5157",
      "editorLineNumber.activeForeground": "#a1a3ab",
      "editorIndentGuide.background": "#3c3f4130",
      "editorIndentGuide.activeBackground": "#3c3f41",
      "editorWidget.background": "#1a1b1f",
      "editorWidget.border": "#3c3f4150",
      "editorGutter.addedBackground": "#5cb870",
      "editorGutter.modifiedBackground": "#4d8ef7",
      "editorGutter.deletedBackground": "#f2556a",
      "minimap.background": "#141518",
      "scrollbarSlider.background": "#ffffff18",
      "scrollbarSlider.hoverBackground": "#ffffff28",
      "scrollbarSlider.activeBackground": "#ffffff38",
    },
  });
}
