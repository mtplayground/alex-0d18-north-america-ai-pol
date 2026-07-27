type FeedFilterBarProps = {
  region: string;
  agency: string;
  status: string;
  category: string;
  onChange: (name: "region" | "agency" | "status" | "category", value: string) => void;
  onClear: () => void;
};

export function FeedFilterBar({ agency, category, onChange, onClear, region, status }: FeedFilterBarProps) {
  const hasFilters = Boolean(region || agency || status || category);

  return (
    <div className="feed-filter-bar" aria-label="Filter policy changes">
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
