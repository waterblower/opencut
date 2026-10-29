import type { Child, Markup } from "@design/jsx/jsx-runtime";

/// A macOS window: a 28px title bar with traffic lights above the body.
/// The body is #080809 and the window casts the menu shadow.
export function Window(props: { width: number; height: number; class?: string; children: Child }): Markup {
  return (
    <div
      class={`overflow-hidden rounded-[10px] bg-[#080809] font-sans text-text shadow-menu ${props.class ?? ""}`}
      style={`width:${props.width}px;height:${props.height}px`}
    >
      <div class="flex h-7 items-center gap-2 border-b border-black bg-[#232326] pl-2">
        <span class="size-3 rounded-full bg-[#ff5f57]" />
        <span class="size-3 rounded-full bg-[#febc2e]" />
        <span class="size-3 rounded-full bg-[#28c840]" />
      </div>
      {props.children}
    </div>
  );
}
