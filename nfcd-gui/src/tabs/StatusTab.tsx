import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { api, type Status } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

export function StatusTab() {
  const { t } = useI18n();
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = () => {
    setError(null);
    api.status().then(setStatus, (e) => setError(String(e)));
  };
  useEffect(load, []);
  useEffect(() => {
    const unlisten = listen("settings-shown", load);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (error) return <p className="text-sm text-destructive">{error}</p>;
  if (!status) return <p className="text-sm text-muted-foreground">{t("st.checking")}</p>;

  const fdaOk = status.full_disk_access === "granted";
  const rows: [string, string][] = [
    [t("st.watcher"), status.watcher],
    [t("st.version"), status.version],
    [t("st.binary"), status.watcher_binary.path ?? t("st.notFound")],
    [t("st.config"), status.config.path],
    [t("st.fda"), status.full_disk_access],
    [t("st.update"), status.latest_version ? `${status.update_status} (${status.latest_version})` : status.update_status],
  ];

  return (
    <div className="flex flex-col gap-4">
      <dl className="grid grid-cols-[10rem_1fr] gap-y-1 text-sm">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="truncate font-mono" title={v}>
              {v}
            </dd>
          </div>
        ))}
      </dl>
      {!fdaOk && (
        <div className="rounded-lg border p-3 text-sm">
          <p className="font-medium">{t("fda.title")}</p>
          <ol className="mt-1 list-decimal pl-5 text-muted-foreground">
            <li>{t("fda.step1")}</li>
            <li>{t("fda.step2")}</li>
            <li>
              {t("fda.step3")} <code className="font-mono">{status.watcher_binary.path}</code>
            </li>
            <li>{t("fda.step4")}</li>
          </ol>
          <Button size="sm" className="mt-2" onClick={() => api.openFdaSettings()}>
            {t("fda.open")}
          </Button>
        </div>
      )}
      <Button variant="outline" className="self-start" onClick={load}>
        {t("st.refresh")}
      </Button>
    </div>
  );
}
