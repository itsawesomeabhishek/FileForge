import type { TargetInfo } from "../api";

interface Props {
  targets: TargetInfo[];
  selected: string | null;
  loading: boolean;
  hasConvertible: boolean;
  onSelect: (format: string) => void;
}

export default function TargetPicker({
  targets,
  selected,
  loading,
  hasConvertible,
  onSelect,
}: Props) {
  if (!hasConvertible) {
    return <p className="muted">None of the selected files can be converted.</p>;
  }
  if (loading) {
    return <p className="muted">Checking available formats…</p>;
  }
  if (targets.length === 0) {
    return (
      <p className="muted">
        There is no format all of the selected files can convert to. Try fewer
        or similar files.
      </p>
    );
  }
  return (
    <div className="targets" role="radiogroup" aria-label="Convert to">
      {targets.map((target) => (
        <button
          key={target.format}
          type="button"
          role="radio"
          aria-checked={selected === target.format}
          className={`chip${selected === target.format ? " is-selected" : ""}`}
          onClick={() => onSelect(target.format)}
        >
          {target.label}
        </button>
      ))}
    </div>
  );
}
