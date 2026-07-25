import { useQuery } from "@tanstack/react-query";

import { getChangeFeed, getEntryDetail, type ChangeFeedFilters } from "./client";

export function useChangeFeed(filters: ChangeFeedFilters = {}) {
  return useQuery({
    queryKey: ["change-feed", filters],
    queryFn: () => getChangeFeed(filters),
  });
}

export function useEntryDetail(entryId: string | undefined) {
  return useQuery({
    queryKey: ["entry-detail", entryId],
    queryFn: () => getEntryDetail(entryId ?? ""),
    enabled: Boolean(entryId),
  });
}
