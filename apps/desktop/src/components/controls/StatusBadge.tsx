export type SourceStatus = "pending" | "indexing" | "ready" | "error";

const STATUS_LABELS: Record<SourceStatus, string> = {
  pending: "Chờ",
  indexing: "Đang phân tích",
  ready: "Sẵn sàng",
  error: "Lỗi",
};

const STATUS_COLORS: Record<SourceStatus, string> = {
  pending: "var(--system-gray)",
  indexing: "var(--system-orange)",
  ready: "var(--system-green)",
  error: "var(--system-red)",
};

export interface StatusBadgeProps {
  status: SourceStatus | string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const normStatus = (status.toLowerCase() as SourceStatus) in STATUS_LABELS
    ? (status.toLowerCase() as SourceStatus)
    : "pending";

  const label = STATUS_LABELS[normStatus] || status;
  const color = STATUS_COLORS[normStatus] || "var(--system-gray)";

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        fontFamily: "var(--font-ui)",
        fontSize: "var(--text-caption1)",
        fontWeight: 500,
        color: color,
        backgroundColor: `color-mix(in srgb, ${color} 14%, transparent)`,
        borderRadius: "var(--radius-sm)",
        padding: "2px 6px",
        lineHeight: 1.2,
        whiteSpace: "nowrap",
        letterSpacing: "0.01em",
      }}
    >
      {label}
    </span>
  );
}
