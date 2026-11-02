import type { Child, Markup } from "@design/jsx/jsx-runtime";

/// A titled mock-up on a design page: a heading, a caption and the mock-up itself.
export function Figure(props: { title: string; caption: string; captionClass?: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-[15px] leading-5 font-semibold text-ink">{props.title}</h2>
      <p class={`mt-[3px] text-xs leading-4 text-dim ${props.captionClass ?? ""}`}>{props.caption}</p>
      <div class="mt-3">{props.children}</div>
    </figure>
  );
}

/// A page section: a rule, a heading and a description, then spaced mock-ups. `gap` is the
/// vertical space between them: `10` or `12`.
export function Section(props: { title: string; description: string; gap: 10 | 12; children: Child }): Markup {
  return (
    <section class="mt-12 border-t border-rule pt-8">
      <h2 class="text-xl leading-7 font-bold">{props.title}</h2>
      <p class="mt-1 text-sm leading-5 text-dim">{props.description}</p>
      <div class={`mt-6 ${props.gap === 10 ? "space-y-10" : "space-y-12"}`}>{props.children}</div>
    </section>
  );
}
