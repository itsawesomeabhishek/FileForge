import type { JobInfo } from "../api";
import { AlertIcon, CategoryIcon, CheckIcon, CrossIcon, FolderIcon, OpenIcon } from "./icons";

interface Props {
  jobs: JobInfo[];
  onCancel: (id: string) => void;
  onOpen: (path: string) => void;
  onReveal: (path: string) => void;
  onDismiss: () => void;
}

function subtitle(job: JobInfo): string {
  switch (job.status) {
    case "waiting":
      return `${job.detail} · Waiting`;
    case "running":
      return `${job.detail} · ${Math.round(job.progress * 100)}%`;
    case "done":
      return `${job.detail} · Done`;
    case "cancelled":
      return `${job.detail} · Cancelled`;
    case "failed":
      return `${job.detail} · ${job.error ?? "Failed"}`;
  }
}

function StatusIcon({ job }: { job: JobInfo }) {
  switch (job.status) {
    case "done":
      return (
        <span className="status status-success">
          <CheckIcon />
        </span>
      );
    case "failed":
      return (
        <span className="status status-error">
          <AlertIcon />
        </span>
      );
    case "cancelled":
      return (
        <span className="status muted">
          <CrossIcon />
        </span>
      );
    default:
      return (
        <span className="status">
          <CategoryIcon category={job.category} />
        </span>
      );
  }
}

export default function JobList({ jobs, onCancel, onOpen, onReveal, onDismiss }: Props) {
  if (jobs.length === 0) return null;
  const finished = jobs.every((job) => job.status !== "waiting" && job.status !== "running");

  return (
    <section className="jobs" aria-label="Conversions">
      <div className="section-header">
        <h2>Conversions</h2>
        {finished && (
          <button type="button" className="link-button" onClick={onDismiss}>
            Clear
          </button>
        )}
      </div>
      <ul className="card job-list">
        {jobs.map((job) => {
          const active = job.status === "waiting" || job.status === "running";
          const output = job.outputs[0];
          return (
            <li key={job.id} className="job-row">
              <StatusIcon job={job} />
              <div className="job-body">
                <div className="job-title">{job.title}</div>
                <div
                  className={`job-detail${job.status === "failed" ? " is-error" : ""}`}
                  title={subtitle(job)}
                >
                  {subtitle(job)}
                </div>
                {active && (
                  <div
                    className="progress"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={Math.round(job.progress * 100)}
                  >
                    <div
                      className="progress-fill"
                      style={{ width: `${Math.round(job.progress * 100)}%` }}
                    />
                  </div>
                )}
              </div>
              <div className="job-actions">
                {active && (
                  <button
                    type="button"
                    className="icon-button"
                    aria-label={`Cancel ${job.detail}`}
                    onClick={() => onCancel(job.id)}
                  >
                    <CrossIcon size={14} />
                  </button>
                )}
                {job.status === "done" && output && (
                  <>
                    <button
                      type="button"
                      className="icon-button"
                      title="Open"
                      aria-label={`Open ${job.detail} output`}
                      onClick={() => onOpen(output)}
                    >
                      <OpenIcon size={14} />
                    </button>
                    <button
                      type="button"
                      className="icon-button"
                      title="Show in Explorer"
                      aria-label={`Show ${job.detail} output in Explorer`}
                      onClick={() => onReveal(output)}
                    >
                      <FolderIcon size={14} />
                    </button>
                  </>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
