import { Link, NavLink, Navigate, Outlet, Route, Routes, useParams } from "react-router-dom";

import { useChangeFeed, useEntryDetail } from "./api/hooks";
import type { ChangeFeedItem } from "./api/client";

const formatDate = (value: string | null) =>
  value ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value)) : "—";

function statusClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized.includes("active") || normalized.includes("force")) return "status-tag is-active";
  if (normalized.includes("draft") || normalized.includes("proposed")) return "status-tag is-draft";
  if (normalized.includes("expired") || normalized.includes("closed"))
    return "status-tag is-closed";
  return "status-tag";
}

function AppShell() {
  return (
    <div className="console-shell">
      <aside className="console-sidebar">
        <Link className="product-mark" to="/">
          Policy workspace
        </Link>
        <nav aria-label="Primary navigation">
          <NavLink end to="/">
            Change feed
          </NavLink>
        </nav>
        <p className="sidebar-note">North America policy monitoring</p>
      </aside>
      <main className="console-main">
        <header className="console-header">
          <div>
            <p className="section-kicker">Monitoring</p>
            <h1>Policy change console</h1>
          </div>
          <span className="live-indicator">Live data</span>
        </header>
        <Outlet />
      </main>
    </div>
  );
}

function FeedPage() {
  const { data, error, isPending } = useChangeFeed({ limit: 40 });

  return (
    <section className="console-panel" aria-labelledby="feed-title">
      <div className="panel-heading">
        <div>
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
            <span role="columnheader">Policy</span>
            <span role="columnheader">Jurisdiction</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">Changed</span>
          </div>
          {data.items.map((item) => (
            <FeedRow item={item} key={`${item.source_url}-${item.changed_at}`} />
          ))}
        </div>
      )}
    </section>
  );
}

function FeedRow({ item }: { item: ChangeFeedItem }) {
  return (
    <a className="feed-row" href={item.source_url} rel="noreferrer" target="_blank">
      <span className="policy-cell">
        <strong>{item.title}</strong>
        <small>{item.agency}</small>
        {item.change_summary && <em>{item.change_summary}</em>}
      </span>
      <span className="region-cell">{item.region.toUpperCase()}</span>
      <span>
        <small className={statusClass(item.status)}>{item.status}</small>
      </span>
      <time dateTime={item.changed_at}>{formatDate(item.changed_at)}</time>
    </a>
  );
}

function EntryDetailPage() {
  const { entryId } = useParams();
  const { data, error, isPending } = useEntryDetail(entryId);

  if (isPending) return <p className="empty-state">Loading entry history…</p>;
  if (error || !data) return <p className="empty-state is-error">Unable to load this entry.</p>;

  return (
    <section className="console-panel" aria-labelledby="entry-title">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">
            {data.entry.region.toUpperCase()} · {data.entry.agency}
          </p>
          <h2 id="entry-title">{data.entry.title}</h2>
        </div>
        <small className={statusClass(data.entry.status)}>{data.entry.status}</small>
      </div>
      <div className="history-list">
        {data.versions.map((version) => (
          <article className="history-item" key={version.id}>
            <div>
              <strong>Version {version.version_number}</strong>
              <span className="version-kind">{version.change_kind}</span>
            </div>
            <time dateTime={version.observed_at}>{formatDate(version.observed_at)}</time>
            <p>{version.change_summary ?? "No summary was available for this observation."}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

export function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<FeedPage />} />
        <Route path="entries/:entryId" element={<EntryDetailPage />} />
        <Route path="*" element={<Navigate replace to="/" />} />
      </Route>
    </Routes>
  );
}
