import * as monaco from "monaco-editor";
import CssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import HtmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import TypeScriptWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

interface MonacoWorkerEnvironment {
  getWorker: (moduleId: string, label: string) => Worker;
}

type MonacoGlobal = typeof globalThis & {
  MonacoEnvironment?: MonacoWorkerEnvironment;
};

let configured = false;

export const configureMonaco = () => {
  if (!configured) {
    (globalThis as MonacoGlobal).MonacoEnvironment = {
      getWorker: (_moduleId, label) => {
        if (label === "json") return new JsonWorker();
        if (["css", "less", "scss"].includes(label)) return new CssWorker();
        if (["html", "handlebars", "razor"].includes(label)) {
          return new HtmlWorker();
        }
        if (["javascript", "typescript"].includes(label)) {
          return new TypeScriptWorker();
        }
        return new EditorWorker();
      },
    };
    configured = true;
  }

  return monaco;
};
