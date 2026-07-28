import { useState } from "react";

import type { ChangeFeedItem } from "../api/client";
import { useEntryDetail } from "../api/hooks";
import { formatDate } from "../formatters";

import { EntryDetailPanel } from "./EntryDetailPanel";
import { ScheduledTag } from "./ScheduledTag";
import { SourceCategoryTag } from "./SourceCategoryTag";
import { StatusTag } from "./StatusTag";

type ChangeFeedRowProps = {
  item: ChangeFeedItem;
  isNewSinceLastVisit: boolean;
  dateKind?: "publication" | "observation";
};

export function ChangeFeedRow({
  dateKind = "publication",
  isNewSinceLastVisit,
  item,
}: ChangeFeedRowProps) {
  const [expanded, setExpanded] = useState(false);
  const detail = useEntryDetail(expanded ? String(item.entry_id) : undefined);
  const displayDate = dateKind === "publication" ? item.publication_date : item.changed_at;
  const detailId = `entry-detail-${item.entry_id}`;

  return (
    <>
      <div
        className={`feed-row${isNewSinceLastVisit ? " is-new-since-last-visit" : ""}`}
        role="row"
      >
        <span className="policy-cell" role="cell">
          <span className="policy-title-line">
            <strong>{item.title}</strong>
            <SourceCategoryTag category={item.source_category} />
            {item.scheduled && <ScheduledTag />}
            {isNewSinceLastVisit && <span className="new-marker">New</span>}
          </span>
          <small>{item.agency}</small>
          <em>{item.change_summary ?? "No change summary was available for this observation."}</em>
        </span>
        <span className="region-cell" role="cell">
          {item.region.toUpperCase()}
        </span>
        <span role="cell">
          <StatusTag status={item.status} />
        </span>
        <time dateTime={displayDate ?? undefined} role="cell">
          {formatDate(displayDate)}
        </time>
        <span className="row-actions" role="cell">
          <button
            aria-controls={detailId}
            aria-expanded={expanded}
            onClick={() => setExpanded((value) => !value)}
            type="button"
          >
            {expanded ? "Hide" : "Details"}
          </button>
          <a className="source-link" href={item.source_url} rel="noreferrer" target="_blank">
            Source <span aria-hidden="true">↗</span>
          </a>
        </span>
      </div>
      {expanded && (
        <div className="expanded-feed-row" id={detailId} role="row">
          <div role="cell">
            {detail.isPending && <p>Loading entry details…</p>}
            {detail.error && <p className="is-error">Unable to load entry details.</p>}
            {detail.data && <EntryDetailPanel detail={detail.data} />}
          </div>
        </div>
      )}
    </>
  );
}
