import { useState } from "react";

import type { ChangeFeedItem } from "../api/client";
import { useEntryDetail } from "../api/hooks";
import { formatDate } from "../formatters";

import { EntryDetailPanel } from "./EntryDetailPanel";
import { StatusTag } from "./StatusTag";

type ChangeFeedRowProps = {
  item: ChangeFeedItem;
};

export function ChangeFeedRow({ item }: ChangeFeedRowProps) {
  const [expanded, setExpanded] = useState(false);
  const detail = useEntryDetail(expanded ? String(item.entry_id) : undefined);
  const displayDate = item.publication_date ?? item.changed_at;
  const detailId = `entry-detail-${item.entry_id}`;

  return (
    <>
      <div className="feed-row" role="row">
        <span className="policy-cell" role="cell">
          <strong>{item.title}</strong>
          <small>{item.agency}</small>
          <em>{item.change_summary ?? "No change summary was available for this observation."}</em>
        </span>
        <span className="region-cell" role="cell">
          {item.region.toUpperCase()}
        </span>
        <span role="cell">
          <StatusTag status={item.status} />
        </span>
        <time dateTime={displayDate} role="cell">
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
