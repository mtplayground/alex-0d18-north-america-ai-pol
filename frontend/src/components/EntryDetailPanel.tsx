import type { EntryDetailResponse } from "../api/client";
import { formatDate } from "../formatters";

import { StatusTag } from "./StatusTag";

type EntryDetailPanelProps = {
  detail: EntryDetailResponse;
};

export function EntryDetailPanel({ detail }: EntryDetailPanelProps) {
  const latestVersion = detail.versions[0];

  return (
    <section className="entry-detail-panel" aria-label={`Details for ${detail.entry.title}`}>
      <div className="detail-metadata">
        <span>
          <small>Agency</small>
          {detail.entry.agency}
        </span>
        <span>
          <small>Region</small>
          {detail.entry.region.toUpperCase()}
        </span>
        <span>
          <small>Published</small>
          {formatDate(detail.entry.publication_date)}
        </span>
        <span>
          <small>Status</small>
          <StatusTag status={detail.entry.status} />
        </span>
      </div>
      <div className="detail-summary">
        <small>What changed</small>
        <p>
          {latestVersion?.change_summary ?? "No change summary was available for this observation."}
        </p>
      </div>
      <div className="detail-diff-slot">
        <span>
          {detail.versions.length} observed version{detail.versions.length === 1 ? "" : "s"}
        </span>
        <span>Version comparison can be added here.</span>
      </div>
    </section>
  );
}
