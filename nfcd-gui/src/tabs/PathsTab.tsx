import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { homeDir } from "@tauri-apps/api/path";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { PatternsEditor } from "@/components/PatternsEditor";
import type { ConfigView, Draft, PathMode } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

type Paths = Draft["paths"];

export function PathsTab({
  draft,
  view,
  rev,
  onChange,
}: {
  draft: Draft;
  view: ConfigView | null;
  rev: number;
  onChange: (paths: Paths) => void;
}) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState<number | null>(null);
  const update = (i: number, patch: Partial<Paths[number]>) =>
    onChange(draft.paths.map((p, j) => (j === i ? { ...p, ...patch } : p)));

  const add = async () => {
    const dir = await open({ directory: true });
    if (!dir) return;
    const home = (await homeDir()).replace(/\/$/, "");
    const raw = dir.startsWith(home + "/") ? "~" + dir.slice(home.length) : dir;
    onChange([...draft.paths, { raw, mode: "recursive", ignore: [] }]);
  };

  return (
    <div className="flex flex-col gap-2">
      {draft.paths.length === 0 && (
        <p className="py-6 text-center text-sm text-muted-foreground">{t("paths.empty")}</p>
      )}
      {draft.paths.map((p, i) => {
        const v = view?.paths[i];
        return (
          <div key={`${rev}-${i}`} className="rounded-lg border p-2">
            <div className="flex items-center gap-2">
              <span className="w-6 text-right text-xs text-muted-foreground">{i + 1}</span>
              <span className="flex-1 truncate font-mono text-sm" title={p.raw}>
                {p.raw}
              </span>
              {v && <StatusBadge status={v.status} redundantOf={v.redundant_of} />}
              <Select value={p.mode} onValueChange={(m) => update(i, { mode: m as PathMode })}>
                <SelectTrigger size="sm" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="recursive">recursive</SelectItem>
                  <SelectItem value="children">children</SelectItem>
                </SelectContent>
              </Select>
              <Button
                size="sm"
                variant={expanded === i ? "secondary" : "ghost"}
                onClick={() => setExpanded(expanded === i ? null : i)}
              >
                {t("paths.ignore")}{p.ignore.length > 0 && ` (${p.ignore.length})`}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="text-destructive"
                onClick={() => {
                  setExpanded(null);
                  onChange(draft.paths.filter((_, j) => j !== i));
                }}
              >
                {t("paths.remove")}
              </Button>
            </div>
            {expanded === i && (
              <div className="mt-2 flex flex-col gap-1 pl-8">
                <PatternsEditor
                  patterns={p.ignore}
                  onChange={(ignore) => update(i, { ignore })}
                  placeholder={"/node_modules/\n*.tmp\n!/keep/"}
                  rows={4}
                />
                <p className="text-xs text-muted-foreground">
                  {t("paths.help")}
                </p>
              </div>
            )}
          </div>
        );
      })}
      <Button variant="outline" className="self-start" onClick={add}>
        {t("paths.add")}
      </Button>
    </div>
  );
}

function StatusBadge({ status, redundantOf }: { status: string; redundantOf: number | null }) {
  const { t } = useI18n();
  if (status === "Active") return <Badge variant="secondary">{t("status.active")}</Badge>;
  if (redundantOf !== null)
    return (
      <Badge variant="outline" title={t("status.redundantTitle", { n: redundantOf + 1 })}>
        {t("status.redundant")} (#{redundantOf + 1})
      </Badge>
    );
  return <Badge variant="destructive">{status.toLowerCase()}</Badge>;
}
