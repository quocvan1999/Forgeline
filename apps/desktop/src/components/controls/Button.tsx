import { forwardRef } from "react";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "destructive" | "toolbar-icon";
  size?: "compact" | "normal";
  iconOnly?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      children,
      variant = "secondary",
      size = "normal",
      iconOnly = false,
      className = "",
      disabled,
      style,
      ...props
    },
    ref,
  ) => {
    const isToolbarIcon = variant === "toolbar-icon" || iconOnly;

    // Base inline styles matching HIG specifications
    const baseStyle: React.CSSProperties = {
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      fontFamily: "var(--font-ui)",
      fontSize: size === "compact" ? "var(--text-callout)" : "var(--text-body)",
      fontWeight: variant === "primary" ? 600 : 400,
      borderRadius: "var(--radius-sm)",
      height: size === "compact" || isToolbarIcon ? "24px" : "28px",
      minWidth: isToolbarIcon ? "24px" : undefined,
      padding: isToolbarIcon ? "0 4px" : "0 10px",
      cursor: disabled ? "default" : "default",
      opacity: disabled ? 0.4 : 1,
      pointerEvents: disabled ? "none" : "auto",
      transition: "background-color var(--motion-duration) var(--motion-ease), border-color var(--motion-duration) var(--motion-ease)",
      userSelect: "none",
      ...style,
    };

    // Style variant specifics
    let variantStyle: React.CSSProperties = {};
    if (variant === "primary") {
      variantStyle = {
        background: "var(--accent)",
        color: "var(--on-accent)",
        border: "none",
      };
    } else if (variant === "secondary") {
      variantStyle = {
        background: "var(--bg-control)",
        color: "var(--label)",
        border: "1px solid var(--separator)",
      };
    } else if (variant === "destructive") {
      variantStyle = {
        background: "transparent",
        color: "var(--system-red)",
        border: "1px solid var(--separator)",
      };
    } else if (variant === "toolbar-icon") {
      variantStyle = {
        background: "transparent",
        color: "var(--label)",
        border: "none",
      };
    }

    return (
      <button
        ref={ref}
        disabled={disabled}
        className={`app-btn app-btn-${variant} ${className}`}
        style={{ ...baseStyle, ...variantStyle }}
        {...props}
      >
        {children}
      </button>
    );
  },
);

Button.displayName = "Button";
