import type { Markup } from "@design/jsx/jsx-runtime";

export type ContextMenuItem = {
  label: string;
  shortcut?: string;
  state?: "default" | "hovered" | "disabled";
  destructive?: boolean;
};

/// A context menu: 3px padding inside a 1px border, then 40px rows with 12px side padding.
export function ContextMenu(props: { width: number; items: ContextMenuItem[] }): Markup {
  return (
    <div
      role="menu"
      class="rounded-lg border border-border bg-menu p-[3px] shadow-menu"
      style={`width:${props.width}px`}
    >
      {props.items.map((item) => {
        const color = item.state === "disabled" ? "text-muted" : item.destructive ? "text-error" : "text-text";
        return (
          <div
            role="menuitem"
            aria-disabled={item.state === "disabled" ? "true" : undefined}
            class={`flex h-10 items-center justify-between rounded-md px-3 text-sm leading-5 ${color} ${
              item.state === "hovered" ? "bg-menu-hover" : ""
            }`}
          >
            <span class="font-sans">{item.label}</span>
            {item.shortcut && <span class="font-mono text-muted">{item.shortcut}</span>}
          </div>
        );
      })}
    </div>
  );
}
