export type ChangeFeedFilters = {
  region?: string;
  agency?: string;
  status?: string;
  limit?: number;
  offset?: number;
};

export type ChangeFeedItem = {
  entry_id: number;
  title: string;
  region: string;
  source_category: "policy" | "news";
  agency: string;
  publication_date: string | null;
  status: string;
  source_url: string;
  change_summary: string | null;
  changed_at: string;
};

export type ChangeFeedResponse = {
  items: ChangeFeedItem[];
  limit: number;
  offset: number;
  next_offset: number | null;
};

export type EntryVersion = {
  id: number;
  version_number: number;
  change_kind: "new" | "updated";
  canonical_content: Record<string, unknown>;
  content_hash: string;
  observed_at: string;
  change_summary: string | null;
};

export type EntryDetail = {
  id: number;
  source_id: number;
  source_external_id: string;
  title: string;
  region: string;
  source_category: "policy" | "news";
  agency: string;
  publication_date: string | null;
  status: string;
  source_url: string;
};

export type EntryDetailResponse = {
  entry: EntryDetail;
  versions: EntryVersion[];
};

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(path, { headers: { Accept: "application/json" } });
  if (!response.ok) {
    throw new ApiError(`Request failed (${response.status})`, response.status);
  }

  return (await response.json()) as T;
}

function queryString(filters: ChangeFeedFilters): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(filters)) {
    if (value !== undefined && value !== "") {
      params.set(key, String(value));
    }
  }

  const query = params.toString();
  return query ? `?${query}` : "";
}

export function getChangeFeed(filters: ChangeFeedFilters = {}): Promise<ChangeFeedResponse> {
  return getJson<ChangeFeedResponse>(`/api/changes${queryString(filters)}`);
}

export function getEntryDetail(entryId: string): Promise<EntryDetailResponse> {
  return getJson<EntryDetailResponse>(`/api/entries/${encodeURIComponent(entryId)}`);
}
