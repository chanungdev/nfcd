import { invoke } from "@tauri-apps/api/core";

export type PathMode = "recursive" | "children";

export interface PathView {
  raw: string;
  mode: PathMode;
  ignore: string[];
  status: string;
  /** 0-based index of the path this one is redundant under. */
  redundant_of: number | null;
}

export interface ConfigView {
  global_ignore: string[];
  paths: PathView[];
}

export interface Draft {
  global_ignore: string[];
  paths: { raw: string; mode: PathMode; ignore: string[] }[];
}

/** One element of `nfcd check-ignore --json`. */
export interface IgnoreCheck {
  path: string;
  status: "ignored" | "not_ignored" | "not_watched";
  entry: { index: number; path: string } | null;
  pattern: string | null;
  source: "global" | "entry" | "nfcignore" | null;
  file: string | null;
}

/** Subset of `nfcd status --json` used by the Status tab. */
export interface Status {
  watcher: string;
  version: string;
  watcher_binary: { path: string | null; exists: boolean };
  config: { path: string; exists: boolean };
  full_disk_access: string;
  update_status: string;
  latest_version: string | null;
}

export const toDraft = (c: ConfigView): Draft => ({
  global_ignore: c.global_ignore,
  paths: c.paths.map(({ raw, mode, ignore }) => ({ raw, mode, ignore })),
});

export const api = {
  load: () => invoke<ConfigView>("load"),
  refresh: (draft: Draft) => invoke<ConfigView>("refresh", { draft }),
  save: (draft: Draft) => invoke<ConfigView>("save", { draft }),
  checkIgnore: (path: string) => invoke<IgnoreCheck | null>("check_ignore", { path }),
  status: () => invoke<Status>("status"),
  openFdaSettings: () => invoke<void>("open_fda_settings"),
};

/** One pattern per line, like a .gitignore file. */
export const linesToPatterns = (text: string) =>
  text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
