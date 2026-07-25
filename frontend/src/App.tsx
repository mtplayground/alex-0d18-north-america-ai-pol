import { Link, NavLink, Navigate, Outlet, Route, Routes, useParams } from "react-router-dom";

import { useEntryDetail } from "./api/hooks";
import { StatusTag } from "./components/StatusTag";
import { formatDate } from "./formatters";
import { ChangeFeedPage } from "./pages/ChangeFeedPage";

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
        <StatusTag status={data.entry.status} />
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
        <Route index element={<ChangeFeedPage />} />
        <Route path="entries/:entryId" element={<EntryDetailPage />} />
        <Route path="*" element={<Navigate replace to="/" />} />
      </Route>
    </Routes>
  );
}
