import { useSearchParams } from "react-router-dom";

import { DEFAULT_CHANGE_FEED_SORT, isChangeFeedSort } from "../api/client";
import { useChangeFeed } from "../api/hooks";
import { ChangeFeedRow } from "../components/ChangeFeedRow";
import { FeedFilterBar } from "../components/FeedFilterBar";
import { changedSince, useLastVisit } from "../lastVisit";

export function ChangeFeedPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const region = searchParams.get("region") ?? "";
  const agency = searchParams.get("agency") ?? "";
  const status = searchParams.get("status") ?? "";
  const categoryParam = searchParams.get("category");
  const category = categoryParam === "policy" || categoryParam === "news" ? categoryParam : "";
  const sortParam = searchParams.get("sort");
  const sort = isChangeFeedSort(sortParam) ? sortParam : DEFAULT_CHANGE_FEED_SORT;
  const sortsByPublicationDate = sort === "published_desc" || sort === "published_asc";
  const lastVisit = useLastVisit();
  const {
    data,
    error,
    fetchNextPage,
    hasNextPage,
    isFetchNextPageError,
    isFetchingNextPage,
    isPending,
  } = useChangeFeed({
    limit: 40,
    sort,
    ...(region && { region }),
    ...(agency && { agency }),
    ...(status && { status }),
    ...(category && { category }),
  });
  const items = data?.pages.flatMap((page) => page.items) ?? [];

  const updateFilter = (
    name: "region" | "agency" | "status" | "category" | "sort",
    value: string,
  ) => {
    const next = new URLSearchParams(searchParams);
    if (value) {
      next.set(name, value);
    } else {
      next.delete(name);
    }
    setSearchParams(next, { replace: true });
  };

  const clearFilters = () => setSearchParams({}, { replace: true });
  const newlyChangedCount = items.filter((item) => changedSince(item.changed_at, lastVisit)).length;

  return (
    <section className="console-panel" aria-labelledby="feed-title">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">What’s new</p>
          <h2 id="feed-title">Latest changes</h2>
          <p>
            {sortsByPublicationDate
              ? "Policy updates ordered by publication date across tracked sources."
              : "Policy updates ordered by crawler observation across tracked sources."}
          </p>
        </div>
        <span className="result-count">
          {data
            ? `${items.length} shown${newlyChangedCount ? ` · ${newlyChangedCount} new` : ""}`
            : "Loading"}
        </span>
      </div>
      <FeedFilterBar
        agency={agency}
        category={category}
        onChange={updateFilter}
        onClear={clearFilters}
        region={region}
        sort={sort}
        status={status}
      />
      {isPending && <p className="empty-state">Loading policy changes…</p>}
      {error && !data && <p className="empty-state is-error">Unable to load the change feed.</p>}
      {data && items.length === 0 && <p className="empty-state">No policy changes yet.</p>}
      {data && items.length > 0 && (
        <div className="feed-table" role="table" aria-label="Latest policy changes">
          <div className="feed-row feed-row-header" role="row">
            <span role="columnheader">Policy and what changed</span>
            <span role="columnheader">Region</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">{sortsByPublicationDate ? "Published" : "Observed"}</span>
            <span role="columnheader">Actions</span>
          </div>
          {items.map((item) => (
            <ChangeFeedRow
              isNewSinceLastVisit={changedSince(item.changed_at, lastVisit)}
              item={item}
              key={`${item.source_url}-${item.changed_at}`}
              dateKind={sortsByPublicationDate ? "publication" : "observation"}
            />
          ))}
        </div>
      )}
      {isFetchNextPageError && <p className="empty-state is-error">Unable to load more changes.</p>}
      {hasNextPage && (
        <div className="feed-pagination">
          <button disabled={isFetchingNextPage} onClick={() => fetchNextPage()} type="button">
            {isFetchingNextPage ? "Loading more…" : "Load more"}
          </button>
        </div>
      )}
    </section>
  );
}
