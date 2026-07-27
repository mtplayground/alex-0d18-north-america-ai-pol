import { useInfiniteQuery, useQuery } from "@tanstack/react-query";

import { getChangeFeed, getEntryDetail, type ChangeFeedFilters } from "./client";

export function useChangeFeed(filters: Omit<ChangeFeedFilters, "offset"> = {}) {
  return useInfiniteQuery({
    queryKey: ["change-feed", filters],
    initialPageParam: 0,
    queryFn: ({ pageParam }) => getChangeFeed({ ...filters, offset: pageParam }),
    getNextPageParam: (lastPage) => lastPage.next_offset ?? undefined,
  });
}

export function useEntryDetail(entryId: string | undefined) {
  return useQuery({
    queryKey: ["entry-detail", entryId],
    queryFn: () => getEntryDetail(entryId ?? ""),
    enabled: Boolean(entryId),
  });
}
