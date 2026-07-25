import { useEffect, useState } from "react";

const LAST_VISIT_STORAGE_KEY = "policy-change-feed.last-visit";

function readLastVisit(): number | null {
  try {
    const value = window.localStorage.getItem(LAST_VISIT_STORAGE_KEY);
    const timestamp = value ? Date.parse(value) : Number.NaN;
    return Number.isNaN(timestamp) ? null : timestamp;
  } catch {
    return null;
  }
}

/** Returns the prior visit timestamp while recording the current client visit. */
export function useLastVisit(): number | null {
  const [lastVisit] = useState(readLastVisit);
  const [currentVisit] = useState(() => new Date().toISOString());

  useEffect(() => {
    try {
      window.localStorage.setItem(LAST_VISIT_STORAGE_KEY, currentVisit);
    } catch {
      // Storage can be unavailable in private or restricted browser contexts.
    }
  }, [currentVisit]);

  return lastVisit;
}

export function changedSince(timestamp: string, lastVisit: number | null): boolean {
  return lastVisit !== null && Date.parse(timestamp) > lastVisit;
}
