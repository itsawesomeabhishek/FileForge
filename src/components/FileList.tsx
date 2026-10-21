import type { FileInfo } from "../api";
import { CategoryIcon, CrossIcon } from "./icons";

interface Props {
  files: FileInfo[];
  onRemove: (path: string) => void;
}

export default function FileList({ files, onRemove }: Props) {
  return (
    <ul className="card file-list">
      {files.map((file) => (
        <li key={file.path} className="file-row">
          <span className="file-icon">
            <CategoryIcon category={file.category} />
          </span>
          <span className="file-name" title={file.path}>
            {file.name}
          </span>
          {file.format ? (
            <span className="badge">{file.format.toUpperCase()}</span>
          ) : (
            <span className="badge badge-error">Unsupported</span>
          )}
          <button
            type="button"
            className="icon-button"
            aria-label={`Remove ${file.name}`}
            onClick={() => onRemove(file.path)}
          >
            <CrossIcon size={14} />
          </button>
        </li>
      ))}
    </ul>
  );
}
