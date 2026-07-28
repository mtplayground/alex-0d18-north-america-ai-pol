import { useEffect, useState } from "react";

import { DEFAULT_CHANGE_FEED_SORT, type ChangeFeedSort } from "../api/client";

const SEARCH_DEBOUNCE_MS = 300;

type FeedFilterBarProps = {
  region: string;
  q: string;
  agency: string;
  status: string;
  category: string;
  sort: ChangeFeedSort;
  onChange: (
    name: "q" | "region" | "agency" | "status" | "category" | "sort",
    value: string,
  ) => void;
  onClear: () => void;
};

export function FeedFilterBar({
  agency,
  category,
  onChange,
  onClear,
  q,
  region,
  sort,
  status,
}: FeedFilterBarProps) {
  const [searchInput, setSearchInput] = useState(q);

  useEffect(() => {
    setSearchInput(q);
  }, [q]);

  useEffect(() => {
    if (searchInput.trim() === q) return undefined;

    const timeoutId = window.setTimeout(() => {
      onChange("q", searchInput.trim());
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timeoutId);
  }, [onChange, q, searchInput]);

  const hasFilters = Boolean(
    searchInput || region || agency || status || category || sort !== DEFAULT_CHANGE_FEED_SORT,
  );

  const clearFilters = () => {
    setSearchInput("");
    onClear();
  };

  return (
    <div className="feed-filter-bar" aria-label="Filter policy changes">
      <label>
        <span>Search changes</span>
        <input
          aria-label="Search policy changes"
          onChange={(event) => setSearchInput(event.target.value)}
          placeholder="Titles or summaries"
          type="search"
          value={searchInput}
        />
      </label>
      <label>
        <span>Sort by</span>
        <select onChange={(event) => onChange("sort", event.target.value)} value={sort}>
          <option value="published_desc">Newest published first</option>
          <option value="published_asc">Oldest published first</option>
          <option value="observed_desc">Newest observed first</option>
          <option value="observed_asc">Oldest observed first</option>
        </select>
      </label>
      <label>
        <span>Region</span>
        <select onChange={(event) => onChange("region", event.target.value)} value={region}>
          <option value="">All regions</option>
          <option value="us">United States</option>
          <option value="ca">Canada</option>
          <option value="global">Global</option>
        </select>
      </label>
      <label>
        <span>Source type</span>
        <select onChange={(event) => onChange("category", event.target.value)} value={category}>
          <option value="">AI news and policy</option>
          <option value="news">AI news</option>
          <option value="policy">Government policy</option>
        </select>
      </label>
      <label>
        <span>Agency</span>
        <input
          onChange={(event) => onChange("agency", event.target.value)}
          placeholder="Any agency"
          type="search"
          value={agency}
        />
      </label>
      <label>
        <span>Status</span>
        <input
          list="policy-statuses"
          onChange={(event) => onChange("status", event.target.value)}
          placeholder="Any status"
          type="search"
          value={status}
        />
        <datalist id="policy-statuses">
          <option value="active" />
          <option value="draft" />
          <option value="proposed" />
          <option value="expired" />
        </datalist>
      </label>
      <button disabled={!hasFilters} onClick={clearFilters} type="button">
        Clear filters
      </button>
    </div>
  );
}
