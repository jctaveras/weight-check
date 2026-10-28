export type Variant = "primary" | "secondary" | "danger" | "ghost";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:brightness-110 active:brightness-95",
  secondary:
    "border border-border bg-surface text-fg hover:bg-surface-muted active:brightness-95",
  danger: "bg-danger text-white hover:brightness-110 active:brightness-95 dark:text-bg",
  ghost: "text-muted hover:bg-surface-muted hover:text-fg",
};

export function buttonClasses(variant: Variant = "primary", size: "sm" | "md" = "md") {
  const sizing = size === "sm" ? "h-8 px-3 text-sm" : "h-10 px-4 text-sm";
  return `inline-flex items-center justify-center gap-2 rounded-md font-medium whitespace-nowrap transition-[background-color,filter,color] duration-150 disabled:cursor-not-allowed disabled:opacity-50 ${sizing} ${VARIANTS[variant]}`;
}
