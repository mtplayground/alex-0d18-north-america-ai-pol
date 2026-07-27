import { DEFAULT_CHANGE_FEED_SORT, type ChangeFeedSort } from "../api/client";

type FeedFilterBarProps = {
  region: string;
  agency: string;
  status: string;
  category: string;
  sort: ChangeFeedSort;
  onChange: (name: "region" | "agency" | "status" | "category" | "sort", value: string) => void;
  onClear: () => void;
};

export function FeedFilterBar({
  agency,
  category,
  onChange,
  onClear,
  region,
  sort,
  status,
}: FeedFilterBarProps) {
  const hasFilters = Boolean(
    region || agency || status || category || sort !== DEFAULT_CHANGE_FEED_SORT,
  );

  return (
    <div className="feed-filter-bar" aria-label="Filter policy changes">
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
      <button disabled={!hasFilters} onClick={onClear} type="button">
        Clear filters
      </button>
    </div>
  );
}
