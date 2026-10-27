// Renders JSX straight to SVG/HTML markup. There is no virtual DOM: every element
// becomes a Markup string as soon as it is created.

export class Markup {
  constructor(readonly html: string) {}
  toString(): string {
    return this.html;
  }
}

export type Child = Markup | string | number | boolean | null | undefined | Child[];
type Props = Record<string, unknown> & { children?: Child };
type Component = (props: Props) => Markup;

export function jsx(type: string | Component, props: Props): Markup {
  if (typeof type === "function") return type(props);
  const { children, ...attributes } = props;
  const attributeText = Object.entries(attributes)
    .filter(([, value]) => value !== undefined && value !== null && value !== false)
    .map(([name, value]) => ` ${name}="${escape(String(value))}"`)
    .join("");
  const content = renderChildren(children);
  if (content === "") return new Markup(`<${type}${attributeText}/>`);
  return new Markup(`<${type}${attributeText}>${content}</${type}>`);
}

export const jsxs = jsx;

export function Fragment(props: Props): Markup {
  return new Markup(renderChildren(props.children));
}

/// Inserts markup as is, e.g. CSS inside <style>.
export function raw(html: string): Markup {
  return new Markup(html);
}

function renderChildren(children: Child): string {
  if (children === undefined || children === null || children === false || children === true) {
    return "";
  }
  if (Array.isArray(children)) return children.map(renderChildren).join("");
  if (children instanceof Markup) return children.html;
  return escape(String(children));
}

function escape(text: string): string {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export declare namespace JSX {
  type Element = Markup;
  interface IntrinsicElements {
    [name: string]: Record<string, unknown>;
  }
  interface ElementChildrenAttribute {
    children: unknown;
  }
}
