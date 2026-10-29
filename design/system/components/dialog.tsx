import type { Child, Markup } from "@design/jsx/jsx-runtime";

/// A dialog panel with the menu shadow. Its padding is measured from the outer edge, so it is
/// 19px inside the 1px border. The title is 18px and semibold unless `plainTitle` is set.
export function Dialog(props: {
  width: number;
  title: string;
  plainTitle?: boolean;
  children: Child;
}): Markup {
  return (
    <section
      role="dialog"
      class="rounded-xl border border-border bg-panel p-[19px] font-sans text-text shadow-menu"
      style={`width:${props.width}px`}
    >
      <h2 class={`text-lg leading-none ${props.plainTitle ? "mt-[3px]" : "mt-1 font-semibold"}`}>{props.title}</h2>
      {props.children}
    </section>
  );
}

/// A dialog button, 40px tall with 15px of padding inside its border.
export function DialogButton(props: { label: string; primary?: boolean }): Markup {
  return (
    <button
      type="button"
      class={`h-10 rounded-lg border px-[15px] text-base leading-none ${
        props.primary ? "border-accent bg-accent text-[#17120a]" : "border-border bg-surface text-text"
      }`}
    >
      {props.label}
    </button>
  );
}

/// A smaller button without a border, used in compact dialogs.
export function DialogTextButton(props: { label: string }): Markup {
  return (
    <button type="button" class="h-9 rounded-md bg-surface px-4 text-base leading-none text-text">
      {props.label}
    </button>
  );
}

/// The row of buttons at the bottom of a dialog, right-aligned with 8px between them.
export function DialogButtons(props: { class?: string; children: Child }): Markup {
  return <div class={`flex justify-end gap-2 ${props.class ?? ""}`}>{props.children}</div>;
}

/// A 54px text field with its text in mono. `selected` highlights all of the text;
/// otherwise a caret follows it.
export function DialogField(props: { value: string; selected?: boolean; class?: string }): Markup {
  return (
    <div
      class={`flex h-[54px] items-center rounded-lg border border-border bg-surface px-[15px] font-mono text-base leading-none ${
        props.class ?? ""
      }`}
    >
      {props.selected ? <span class="h-[22px] bg-[#3a5a8c]/90 leading-[22px]">{props.value}</span> : (
        <>
          <span>{props.value}</span>
          <span class="ml-[0.7px] h-5 w-[1.5px] bg-accent" />
        </>
      )}
    </div>
  );
}

/// A dialog header: the title with a close button, above a full-width divider.
export function DialogHeader(props: { title: string }): Markup {
  return (
    <header class="flex h-[57px] items-center justify-between border-b border-border pr-[19px] pl-[19px]">
      <h2 class="text-lg leading-none font-semibold">{props.title}</h2>
      <span class="grid size-8 place-items-center rounded-md bg-hover text-base leading-none">×</span>
    </header>
  );
}

/// A choice in a list of options. The selected one has the accent border and dot.
export function DialogOption(props: { label: string; selected?: boolean }): Markup {
  return (
    <div
      class={`flex h-11 items-center justify-between rounded-lg border px-[11px] text-base leading-none ${
        props.selected ? "border-accent bg-accent-background" : "border-border bg-surface"
      }`}
    >
      <span>{props.label}</span>
      <span class={`size-2 rounded-full ${props.selected ? "bg-accent" : "bg-rail"}`} />
    </div>
  );
}
