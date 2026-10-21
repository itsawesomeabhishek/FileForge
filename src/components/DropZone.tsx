import { UploadIcon } from "./icons";

interface Props {
  hovering: boolean;
  onBrowse: () => void;
}

export default function DropZone({ hovering, onBrowse }: Props) {
  return (
    <button
      type="button"
      className={`drop-zone${hovering ? " is-hovering" : ""}`}
      onClick={onBrowse}
    >
      <UploadIcon size={28} />
      <span className="drop-title">Drop files here</span>
      <span className="drop-hint">or click to choose files</span>
    </button>
  );
}
