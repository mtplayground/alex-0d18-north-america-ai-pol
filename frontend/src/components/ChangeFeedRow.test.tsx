import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import fixture from "../../../tests/fixtures/federal-register-ai-policy.json";

import { ChangeFeedRow } from "./ChangeFeedRow";

describe("ChangeFeedRow", () => {
  it("renders the fixture-backed feed item with its summary and new marker", () => {
    const record = fixture.results[0];
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    render(
      <QueryClientProvider client={queryClient}>
        <ChangeFeedRow
          isNewSinceLastVisit
          item={{
            entry_id: 1,
            title: record.title,
            region: "us",
            source_category: "policy",
            agency: record.agency_names[0],
            publication_date: record.publication_date,
            status: record.status,
            source_url: record.html_url,
            change_summary: "Fixture summary of the policy change.",
            changed_at: "2026-07-25T12:00:00Z",
          }}
        />
      </QueryClientProvider>,
    );

    expect(screen.getByText("Fixture AI Policy Update")).toBeTruthy();
    expect(screen.getByText("Fixture summary of the policy change.")).toBeTruthy();
    expect(screen.getByText("Government policy")).toBeTruthy();
    expect(screen.getByText("New")).toBeTruthy();
    expect(screen.getByRole("link", { name: /source/i })).toHaveProperty(
      "href",
      "https://example.test/policies/e2e-2026-0001",
    );
  });

  it("shows the crawler observation date when the observed sort is active", () => {
    const record = fixture.results[0];
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const { container } = render(
      <QueryClientProvider client={queryClient}>
        <ChangeFeedRow
          dateKind="observation"
          isNewSinceLastVisit={false}
          item={{
            entry_id: 2,
            title: record.title,
            region: "us",
            source_category: "policy",
            agency: record.agency_names[0],
            publication_date: record.publication_date,
            status: record.status,
            source_url: record.html_url,
            change_summary: null,
            changed_at: "2026-07-25T12:00:00Z",
          }}
        />
      </QueryClientProvider>,
    );

    expect(container.querySelector("time")?.getAttribute("datetime")).toBe("2026-07-25T12:00:00Z");
  });
});
