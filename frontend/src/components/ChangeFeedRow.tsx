import type { ChangeFeedItem } from "../api/client";
import { formatDate } from "../formatters";

import { StatusTag } from "./StatusTag";

type ChangeFeedRowProps = {
  item: ChangeFeedItem;
};

export function ChangeFeedRow({ item }: ChangeFeedRowProps) {
  const displayDate = item.publication_date ?? item.changed_at;

  return (
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
      <span role="cell">
        <a className="source-link" href={item.source_url} rel="noreferrer" target="_blank">
          Source <span aria-hidden="true">↗</span>
        </a>
      </span>
    </div>
  );
}
