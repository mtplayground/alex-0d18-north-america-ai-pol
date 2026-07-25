import { useChangeFeed } from "../api/hooks";
import { ChangeFeedRow } from "../components/ChangeFeedRow";

export function ChangeFeedPage() {
  const { data, error, isPending } = useChangeFeed({ limit: 40 });

  return (
    <section className="console-panel" aria-labelledby="feed-title">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">What’s new</p>
          <h2 id="feed-title">Latest changes</h2>
          <p>Newest observed policy updates across tracked sources.</p>
        </div>
        <span className="result-count">{data ? `${data.items.length} shown` : "Loading"}</span>
      </div>
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
            <span role="columnheader">Source</span>
          </div>
          {data.items.map((item) => (
            <ChangeFeedRow item={item} key={`${item.source_url}-${item.changed_at}`} />
          ))}
        </div>
      )}
    </section>
  );
}
