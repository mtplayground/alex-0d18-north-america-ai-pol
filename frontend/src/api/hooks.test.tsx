import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useChangeFeed } from "./hooks";

const firstPage = {
  items: [],
  limit: 40,
  offset: 0,
  next_offset: 40,
};
const secondPage = {
  items: [],
  limit: 40,
  offset: 40,
  next_offset: null,
};

describe("useChangeFeed", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("uses next_offset while preserving the active filters and sort", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify(firstPage), { status: 200 }))
      .mockResolvedValueOnce(new Response(JSON.stringify(secondPage), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(
      () =>
        useChangeFeed({
          limit: 40,
          q: "artificial intelligence",
          region: "ca",
          sort: "observed_asc",
        }),
      { wrapper },
    );

    await waitFor(() => expect(result.current.data?.pages).toHaveLength(1));
    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.pages).toHaveLength(2));

    const firstRequest = new URL(String(fetchMock.mock.calls[0][0]), "https://app.example.test");
    const secondRequest = new URL(String(fetchMock.mock.calls[1][0]), "https://app.example.test");
    expect(firstRequest.searchParams.get("offset")).toBe("0");
    expect(secondRequest.searchParams.get("offset")).toBe("40");
    expect(secondRequest.searchParams.get("limit")).toBe("40");
    expect(secondRequest.searchParams.get("q")).toBe("artificial intelligence");
    expect(secondRequest.searchParams.get("region")).toBe("ca");
    expect(secondRequest.searchParams.get("sort")).toBe("observed_asc");
  });
});
