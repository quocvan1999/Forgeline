export interface SegmentOption<T extends string> {
  id: T;
  label: string;
  badge?: number;
}

export interface SegmentedControlProps<T extends string> {
  options: SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
  size?: "compact" | "normal";
}

export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  disabled = false,
  size = "compact",
}: SegmentedControlProps<T>) {
  return (
    <div
      role="tablist"
      aria-orientation="horizontal"
      style={{
        display: "inline-flex",
        alignItems: "center",
        backgroundColor: "var(--bg-hover)",
        borderRadius: "var(--radius-md)",
        padding: "2px",
        height: size === "compact" ? "26px" : "30px",
        opacity: disabled ? 0.45 : 1,
        pointerEvents: disabled ? "none" : "auto",
        userSelect: "none",
        border: "1px solid var(--separator)",
      }}
    >
      {options.map((option) => {
        const isSelected = option.id === value;
        return (
          <button
            key={option.id}
            role="tab"
            type="button"
            aria-selected={isSelected}
            tabIndex={isSelected ? 0 : -1}
            disabled={disabled}
            onClick={() => onChange(option.id)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "4px",
              padding: "0 10px",
              height: "100%",
              borderRadius: "calc(var(--radius-md) - 2px)",
              fontSize: "var(--text-callout)",
              fontWeight: isSelected ? 600 : 400,
              color: isSelected ? "var(--label)" : "var(--secondary-label)",
              backgroundColor: isSelected ? "var(--bg-content)" : "transparent",
              boxShadow: isSelected
                ? "0 1px 3px rgba(0,0,0,0.12), 0 0 1px rgba(0,0,0,0.08)"
                : "none",
              border: "none",
              cursor: disabled ? "default" : "default",
              transition:
                "background-color var(--motion-duration) var(--motion-ease), color var(--motion-duration) var(--motion-ease), box-shadow var(--motion-duration) var(--motion-ease)",
              whiteSpace: "nowrap",
            }}
          >
            <span>{option.label}</span>
            {option.badge !== undefined && option.badge > 0 && (
              <span
                style={{
                  fontSize: "var(--text-caption2)",
                  padding: "1px 4px",
                  borderRadius: "4px",
                  backgroundColor: isSelected
                    ? "var(--bg-selected)"
                    : "var(--quaternary-label)",
                  color: isSelected ? "var(--accent)" : "var(--secondary-label)",
                }}
              >
                {option.badge}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
