import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { PatternsEditor } from "@/components/PatternsEditor";
import { PathsTab } from "@/tabs/PathsTab";
import { CheckTab } from "@/tabs/CheckTab";
import { StatusTab } from "@/tabs/StatusTab";
import { api, toDraft, type ConfigView, type Draft } from "@/lib/api";
import { useI18n, type Lang } from "@/lib/i18n";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export default function App() {
  const { t, lang, setLang } = useI18n();
  const [draft, setDraft] = useState<Draft | null>(null);
  const [saved, setSaved] = useState("");
  const [view, setView] = useState<ConfigView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  // Bumped on load/revert so the uncontrolled pattern editors reset.
  const [rev, setRev] = useState(0);

  const apply = (c: ConfigView) => {
    const d = toDraft(c);
    setDraft(d);
    setSaved(JSON.stringify(d));
    setView(c);
    setRev((r) => r + 1);
  };
  const load = () => {
    setMessage(null);
    api.load().then(
      (c) => {
        setLoadError(null);
        apply(c);
      },
      (e) => setLoadError(String(e)),
    );
  };
  useEffect(load, []);

  const edit = (d: Draft) => {
    setDraft(d);
    setMessage(null);
    api.refresh(d).then(setView);
  };
  const save = () => {
    if (!draft) return;
    api.save(draft).then(
      (c) => {
        apply(c);
        setMessage(t("footer.saved"));
      },
      (e) => setMessage(String(e)),
    );
  };

  const dirty = draft !== null && JSON.stringify(draft) !== saved;

  return (
    <main className="flex h-screen flex-col gap-3 p-4">
      <Tabs defaultValue="paths" className="min-h-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <TabsList>
            <TabsTrigger value="paths">{t("tab.paths")}</TabsTrigger>
            <TabsTrigger value="global">{t("tab.global")}</TabsTrigger>
            <TabsTrigger value="check">{t("tab.check")}</TabsTrigger>
            <TabsTrigger value="status">{t("tab.status")}</TabsTrigger>
          </TabsList>
          <Select value={lang} onValueChange={(l) => setLang(l as Lang)}>
            <SelectTrigger size="sm" aria-label={t("language")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="ko">한국어</SelectItem>
              <SelectItem value="en">English</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto pt-2">
          {loadError ? (
            <ConfigError error={loadError} onRetry={load} />
          ) : (
            draft && (
              <>
                <TabsContent value="paths">
                  <PathsTab
                    draft={draft}
                    view={view}
                    rev={rev}
                    onChange={(paths) => edit({ ...draft, paths })}
                  />
                </TabsContent>
                <TabsContent value="global" className="flex flex-col gap-2">
                  <p className="text-sm text-muted-foreground">{t("global.help")}</p>
                  <PatternsEditor
                    key={rev}
                    patterns={draft.global_ignore}
                    onChange={(global_ignore) => edit({ ...draft, global_ignore })}
                    placeholder={".git/\nnode_modules/\n*.app/"}
                    rows={16}
                  />
                </TabsContent>
              </>
            )
          )}
          <TabsContent value="check">
            <CheckTab />
          </TabsContent>
          <TabsContent value="status">
            <StatusTab />
          </TabsContent>
        </div>
      </Tabs>
      <footer className="flex items-center gap-2 border-t pt-3">
        <p className="flex-1 truncate text-xs text-muted-foreground">{message}</p>
        <Button variant="outline" disabled={!dirty} onClick={load}>
          {t("footer.revert")}
        </Button>
        <Button disabled={!dirty || loadError !== null} onClick={save}>
          {t("footer.save")}
        </Button>
      </footer>
    </main>
  );
}

function ConfigError({ error, onRetry }: { error: string; onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-destructive/50 p-3 text-sm">
      <p className="font-medium text-destructive">{t("error.title")}</p>
      <p className="font-mono text-xs whitespace-pre-wrap">{error}</p>
      <p className="text-muted-foreground">{t("error.help")}</p>
      <Button size="sm" variant="outline" className="self-start" onClick={onRetry}>
        {t("error.retry")}
      </Button>
    </div>
  );
}
