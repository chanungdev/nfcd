import { Textarea } from "@/components/ui/textarea";
import { linesToPatterns } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

const SYNTAX = [
  ["node_modules/", "syntax.dirAny"],
  ["*.tmp", "syntax.glob"],
  ["/build/", "syntax.anchor"],
  ["a/b/", "syntax.middle"],
  ["!/build/keep.txt", "syntax.negate"],
  ["# note", "syntax.comment"],
] as const;

// Uncontrolled on purpose: the parent stores trimmed patterns, which would eat
// the empty line the user just typed. Remount (change `key`) to reset it.
export function PatternsEditor({
  patterns,
  onChange,
  placeholder,
  rows = 6,
}: {
  patterns: string[];
  onChange: (patterns: string[]) => void;
  placeholder?: string;
  rows?: number;
}) {
  const { t } = useI18n();
  return (
    <div className="flex flex-col gap-2">
      <Textarea
        defaultValue={patterns.join("\n")}
        onChange={(e) => onChange(linesToPatterns(e.target.value))}
        placeholder={placeholder}
        rows={rows}
        spellCheck={false}
        className="font-mono text-xs"
      />
      <dl className="grid grid-cols-[9rem_1fr] gap-x-3 gap-y-0.5 text-xs text-muted-foreground">
        {SYNTAX.map(([pattern, meaning]) => (
          <div key={pattern} className="contents">
            <dt className="font-mono text-foreground">{pattern}</dt>
            <dd>{t(meaning)}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
