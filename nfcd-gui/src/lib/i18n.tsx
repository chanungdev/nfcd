import { createContext, useContext, useState, type ReactNode } from "react";

const en = {
  "tab.paths": "Paths",
  "tab.global": "Global ignore",
  "tab.check": "Check path",
  "tab.status": "Status",
  "footer.revert": "Revert",
  "footer.save": "Save",
  "footer.saved": "Saved. The watcher reloads the config automatically.",
  "error.title": "Can't read config.toml",
  "error.help": "Fix the file by hand, then retry. Saving is disabled so the file isn't overwritten.",
  "error.retry": "Retry",
  "global.help":
    "Applied to every watched folder. A folder's own patterns take precedence. One pattern per line, gitignore syntax.",
  "paths.empty": "No watched folders yet.",
  "paths.ignore": "Ignore",
  "paths.remove": "Remove",
  "paths.add": "+ Add folder…",
  "paths.help": "One pattern per line, relative to this folder. Global patterns also apply.",
  "status.active": "active",
  "status.redundant": "redundant",
  "status.redundantTitle": "Already covered by #{n}",
  "syntax.dirAny": "folder with this name at any depth (trailing / = folders only)",
  "syntax.glob": "files matching the wildcard at any depth; ** matches across folders",
  "syntax.anchor": "leading / anchors to the watched folder: only <folder>/build",
  "syntax.middle": "a / in the middle also anchors: same as /a/b/",
  "syntax.negate": "! re-includes a match (not inside an ignored folder)",
  "syntax.comment": "comment",
  "check.help": "Check whether a path is ignored, and which rule decides it. Uses the saved config.",
  "check.check": "Check",
  "check.file": "File…",
  "check.folder": "Folder…",
  "check.ignored": "Ignored",
  "check.ignoredBy": 'by "{pattern}" from {from}',
  "check.global": "global patterns",
  "check.notIgnored": "Not ignored",
  "check.watchedBy": "watched by {entry}",
  "check.notWatched": "Not watched",
  "check.notWatchedDetail": "no watched folder contains this path",
  "st.checking": "Checking…",
  "st.watcher": "Watcher",
  "st.version": "Version",
  "st.binary": "Watcher binary",
  "st.notFound": "not found",
  "st.config": "Config",
  "st.fda": "Full Disk Access",
  "st.update": "Update",
  "st.refresh": "Refresh",
  "fda.title": "Grant Full Disk Access to the watcher",
  "fda.step1": "Open Full Disk Access settings.",
  "fda.step2": 'If nfcd-watcher is listed, remove it with "−" (not just toggle it off).',
  "fda.step3": 'Click "+", press ⌘⇧G, and paste:',
  "fda.step4": "Come back and press Refresh. Repeat after every upgrade.",
  "fda.open": "Open Full Disk Access settings",
  language: "Language",
};

type Key = keyof typeof en;

const ko: Record<Key, string> = {
  "tab.paths": "경로",
  "tab.global": "Global ignore",
  "tab.check": "경로 확인",
  "tab.status": "상태",
  "footer.revert": "되돌리기",
  "footer.save": "저장",
  "footer.saved": "저장했습니다. watcher가 설정을 자동으로 다시 읽습니다.",
  "error.title": "config.toml을 읽을 수 없습니다",
  "error.help": "파일을 직접 고친 뒤 다시 시도하세요. 파일을 덮어쓰지 않도록 저장은 막아 두었습니다.",
  "error.retry": "다시 시도",
  "global.help":
    "모든 감시 폴더에 적용됩니다. 폴더별 패턴이 우선합니다. 한 줄에 패턴 하나, gitignore 문법입니다.",
  "paths.empty": "감시 중인 폴더가 없습니다.",
  "paths.ignore": "Ignore",
  "paths.remove": "삭제",
  "paths.add": "+ 폴더 추가…",
  "paths.help": "한 줄에 패턴 하나, 이 폴더 기준 상대 경로입니다. global 패턴도 함께 적용됩니다.",
  "status.active": "active",
  "status.redundant": "redundant",
  "status.redundantTitle": "#{n}이 이미 감시 중",
  "syntax.dirAny": "모든 깊이에서 이 이름의 폴더 (끝의 / = 폴더만)",
  "syntax.glob": "모든 깊이에서 와일드카드와 일치하는 파일, **는 여러 폴더에 걸쳐 일치",
  "syntax.anchor": "앞의 /는 감시 폴더 기준으로 고정: <폴더>/build만",
  "syntax.middle": "중간에 /가 있어도 고정: /a/b/와 같음",
  "syntax.negate": "!는 ignore된 것을 다시 포함 (ignore된 폴더 안은 불가)",
  "syntax.comment": "주석",
  "check.help": "경로가 ignore되는지, 어떤 규칙 때문인지 확인합니다. 저장된 설정 기준입니다.",
  "check.check": "확인",
  "check.file": "파일…",
  "check.folder": "폴더…",
  "check.ignored": "Ignore됨",
  "check.ignoredBy": '{from}의 "{pattern}" 패턴',
  "check.global": "global 패턴",
  "check.notIgnored": "Ignore 안 됨",
  "check.watchedBy": "{entry}에서 감시 중",
  "check.notWatched": "감시 대상 아님",
  "check.notWatchedDetail": "이 경로를 포함하는 감시 폴더가 없습니다",
  "st.checking": "확인 중…",
  "st.watcher": "Watcher",
  "st.version": "버전",
  "st.binary": "Watcher 실행 파일",
  "st.notFound": "찾을 수 없음",
  "st.config": "설정 파일",
  "st.fda": "전체 디스크 접근 권한",
  "st.update": "업데이트",
  "st.refresh": "새로고침",
  "fda.title": "watcher에 전체 디스크 접근 권한을 주세요",
  "fda.step1": "전체 디스크 접근 권한 설정을 엽니다.",
  "fda.step2": "nfcd-watcher가 목록에 있으면 끄지 말고 \"−\"로 완전히 삭제합니다.",
  "fda.step3": "\"+\"를 누르고 ⌘⇧G로 아래 경로를 붙여 넣습니다:",
  "fda.step4": "돌아와서 새로고침을 누릅니다. 업그레이드할 때마다 반복해야 합니다.",
  "fda.open": "전체 디스크 접근 권한 설정 열기",
  language: "언어",
};

export type Lang = "en" | "ko";
const DICTS: Record<Lang, Record<Key, string>> = { en, ko };
const STORAGE_KEY = "nfcd.lang";

const initialLang = (): Lang => {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === "en" || saved === "ko") return saved;
  return navigator.language.startsWith("ko") ? "ko" : "en";
};

type T = (key: Key, vars?: Record<string, string | number>) => string;
const I18n = createContext<{ lang: Lang; setLang: (l: Lang) => void; t: T } | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState(initialLang);
  const setLang = (l: Lang) => {
    localStorage.setItem(STORAGE_KEY, l);
    setLangState(l);
  };
  const t: T = (key, vars = {}) =>
    DICTS[lang][key].replace(/\{(\w+)\}/g, (_, v) => String(vars[v] ?? `{${v}}`));
  return <I18n.Provider value={{ lang, setLang, t }}>{children}</I18n.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18n);
  if (!ctx) throw new Error("useI18n outside I18nProvider");
  return ctx;
}
