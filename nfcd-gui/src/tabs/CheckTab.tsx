import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api, type IgnoreCheck } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

export function CheckTab() {
  const { t } = useI18n();
  const [path, setPath] = useState("");
  const [result, setResult] = useState<IgnoreCheck | null>(null);
  const [error, setError] = useState<string | null>(null);

  const check = async (p = path) => {
    if (!p.trim()) return;
    setError(null);
    try {
      setResult(await api.checkIgnore(p.trim()));
    } catch (e) {
      setResult(null);
      setError(String(e));
    }
  };
  const pick = async (directory: boolean) => {
    const p = await open({ directory });
    if (!p) return;
    setPath(p);
    check(p);
  };

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">
        {t("check.help")}
      </p>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          check();
        }}
      >
        <Input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="~/Downloads/project/node_modules"
          className="font-mono"
          spellCheck={false}
        />
        <Button type="submit">{t("check.check")}</Button>
        <Button type="button" variant="outline" onClick={() => pick(false)}>
          {t("check.file")}
        </Button>
        <Button type="button" variant="outline" onClick={() => pick(true)}>
          {t("check.folder")}
        </Button>
      </form>
      {error && <p className="text-sm text-destructive">{error}</p>}
      {result && <CheckResult r={result} />}
    </div>
  );
}

function CheckResult({ r }: { r: IgnoreCheck }) {
  const { t } = useI18n();
  const entry = r.entry && `#${r.entry.index} ${r.entry.path}`;
  const from = r.source === "global" ? t("check.global") : r.source === "nfcignore" ? r.file : entry;
  const [title, detail] =
    r.status === "ignored"
      ? [t("check.ignored"), t("check.ignoredBy", { pattern: r.pattern ?? "", from: from ?? "" })]
      : r.status === "not_ignored"
        ? [t("check.notIgnored"), t("check.watchedBy", { entry: entry ?? "" })]
        : [t("check.notWatched"), t("check.notWatchedDetail")];
  return (
    <div className="rounded-lg border p-3">
      <p className="font-medium">{title}</p>
      <p className="text-sm text-muted-foreground">{detail}</p>
      <p className="mt-1 truncate font-mono text-xs text-muted-foreground">{r.path}</p>
    </div>
  );
}
