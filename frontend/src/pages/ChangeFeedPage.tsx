import { useSearchParams } from "react-router-dom";

import { useChangeFeed } from "../api/hooks";
import { ChangeFeedRow } from "../components/ChangeFeedRow";
import { FeedFilterBar } from "../components/FeedFilterBar";
import { changedSince, useLastVisit } from "../lastVisit";

export function ChangeFeedPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const region = searchParams.get("region") ?? "";
  const agency = searchParams.get("agency") ?? "";
  const status = searchParams.get("status") ?? "";
  const lastVisit = useLastVisit();
  const { data, error, isPending } = useChangeFeed({
    limit: 40,
    ...(region && { region }),
    ...(agency && { agency }),
    ...(status && { status }),
  });

  const updateFilter = (name: "region" | "agency" | "status", value: string) => {
    const next = new URLSearchParams(searchParams);
    if (value) {
      next.set(name, value);
    } else {
      next.delete(name);
    }
    setSearchParams(next, { replace: true });
  };

  const clearFilters = () => setSearchParams({}, { replace: true });
  const newlyChangedCount =
    data?.items.filter((item) => changedSince(item.changed_at, lastVisit)).length ?? 0;

  return (
    <section className="console-panel" aria-labelledby="feed-title">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">What’s new</p>
          <h2 id="feed-title">Latest changes</h2>
          <p>Newest observed policy updates across tracked sources.</p>
        </div>
        <span className="result-count">
          {data
            ? `${data.items.length} shown${newlyChangedCount ? ` · ${newlyChangedCount} new` : ""}`
            : "Loading"}
        </span>
      </div>
      <FeedFilterBar
        agency={agency}
        onChange={updateFilter}
        onClear={clearFilters}
        region={region}
        status={status}
      />
      {isPending && <p className="empty-state">Loading policy changes…</p>}
      {error && <p className="empty-state is-error">Unable to load the change feed.</p>}
      {data && data.items.length === 0 && <p className="empty-state">No policy changes yet.</p>}
      {data && data.items.length > 0 && (
        <div className="feed-table" role="table" aria-label="Latest policy changes">
          <div className="feed-row feed-row-header" role="row">
            <span role="columnheader">Policy and what changed</span>
            <span role="columnheader">Region</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">Date</span>
            <span role="columnheader">Actions</span>
          </div>
          {data.items.map((item) => (
            <ChangeFeedRow
              isNewSinceLastVisit={changedSince(item.changed_at, lastVisit)}
              item={item}
              key={`${item.source_url}-${item.changed_at}`}
            />
          ))}
        </div>
      )}
    </section>
  );
}
