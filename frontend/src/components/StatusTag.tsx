type StatusTagProps = {
  status: string;
};

function statusClass(status: string) {
  const normalized = status.toLowerCase();
  if (normalized.includes("active") || normalized.includes("force")) return "status-tag is-active";
  if (normalized.includes("draft") || normalized.includes("proposed")) return "status-tag is-draft";
  if (normalized.includes("expired") || normalized.includes("closed"))
    return "status-tag is-closed";
  return "status-tag";
}

export function StatusTag({ status }: StatusTagProps) {
  return <small className={statusClass(status)}>{status}</small>;
}
