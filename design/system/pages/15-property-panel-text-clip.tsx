import { ColorPalette, FontSelector, TextClipPanel, type TextClipState } from "../components/text-clip-panel.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const BACKGROUND = { hex: "000000", opacity: "50" };

/// A crop of the panel: `height` rows starting `top` rows down, with a rounded outline.
type Crop = { top: number; height: number };
const CROP = {
  content: { top: 64, height: 110 },
  hex: { top: 272, height: 170 },
  palette: { top: 89, height: 353 },
  font: { top: 52, height: 450 },
  background: { top: 442, height: 82 },
} satisfies Record<string, Crop>;

export default function TextClipPropertyPanelPage(): Markup {
  return (
    <main class="relative h-[2012px] w-[1936px] bg-page px-12 pt-[40px] font-sans text-text">
      <h1 class="text-2xl leading-none font-bold">Property Panel · Text clip</h1>
      <p class="mt-[14px] text-sm leading-none text-muted">
        Every state of the text clip panel, side by side. Other panels are on the property panel page.
      </p>
      <Legend />

      <Section
        top={165}
        title="Text clip"
        caption="Clip states change the whole panel. The section rows below show only the part that changes."
      >
        <Row>
          <View label="Default">
            <TextClipPanel shift={-8} />
          </View>
          <View label="Locked track · read-only">
            <TextClipPanel shift={-8} locked />
          </View>
        </Row>
      </Section>

      <Section top={935} title="Content" caption="Click to edit; the caret and accent border show focus.">
        <Row>
          <View label="Idle">
            <Cropped crop={CROP.content}>
              <TextClipPanel shift={0} background={BACKGROUND} />
            </Cropped>
          </View>
          <View label="Editing">
            <Cropped crop={CROP.content}>
              <TextClipPanel shift={0} background={BACKGROUND} editingContent />
            </Cropped>
          </View>
        </Row>
      </Section>

      <Section top={1201} title="Style" caption="Font, size and font color, with their popovers.">
        <Row>
          <View label="Hex input · focused" note="Fixed # prefix · edit six hex digits only.">
            <Cropped crop={CROP.hex}>
              <TextClipPanel shift={0} background={BACKGROUND} hexFocused />
            </Cropped>
          </View>
          <View label="Color palette · open" note="Select to explore · × / outside closes.">
            <Cropped crop={CROP.palette}>
              <TextClipPanel shift={0} background={BACKGROUND} {...ACCENT_COLOR} overlay={<ColorPalette />} />
            </Cropped>
          </View>
          <View label="Color palette · swatch hover" note="2 px hover ring · checkmark stays on selection.">
            <Cropped crop={CROP.palette}>
              <TextClipPanel shift={0} background={BACKGROUND} {...ACCENT_COLOR} overlay={<ColorPalette hover />} />
            </Cropped>
          </View>
          <View label="Font selector · preview cards" note="Floating overlay · click applies and stays open.">
            <Cropped crop={CROP.font}>
              <TextClipPanel shift={0} background={BACKGROUND} font="PingFang SC" overlay={<FontSelector />} />
            </Cropped>
          </View>
          <View label="Font dropdown · hover" note="Gold accent · warm fill, gold border and chevron.">
            <Cropped crop={CROP.hex}>
              <TextClipPanel shift={0} background={BACKGROUND} fontHover />
            </Cropped>
          </View>
        </Row>
      </Section>

      <Section top={1807} title="Background" caption="None by default. Add sets a color and opacity. Remove clears it.">
        <Row>
          <View label="No background">
            <Cropped crop={CROP.background}>
              <TextClipPanel shift={0} editingContent />
            </Cropped>
          </View>
          <View label="Set">
            <Cropped crop={CROP.background}>
              <TextClipPanel shift={0} editingContent background={BACKGROUND} />
            </Cropped>
          </View>
        </Row>
      </Section>
    </main>
  );
}

const ACCENT_COLOR: Partial<TextClipState> = { colorAccent: true, colorFill: "#f0b75e", colorHex: "F0B75E" };

function Legend(): Markup {
  return (
    <div class="absolute top-[104px] left-12 text-[13px] leading-none">
      <span class="absolute top-0 left-0 h-[30px] w-24 rounded-md border border-border bg-surface" />
      <span class="absolute top-[9px] left-[10px] text-xs font-semibold text-muted">X</span>
      <span class="absolute top-[9px] left-[25.68px] font-mono">0.50</span>
      <span class="absolute top-[9px] left-[106px] whitespace-nowrap text-soft">Editable input</span>
      <span class="absolute top-0 left-[218.86px] h-[30px] w-24 rounded-md bg-well" />
      <span class="absolute top-[9px] left-[228.86px] font-mono">0:08.00</span>
      <span class="absolute top-[9px] left-[324.86px] whitespace-nowrap text-soft">Read-only value</span>
      <span class="absolute top-[9px] left-[453.71px] whitespace-nowrap text-accent">Change…</span>
      <span class="absolute top-[9px] left-[514.15px] whitespace-nowrap text-soft">Action</span>
    </div>
  );
}

/// A titled group of views.
function Section(props: { top: number; title: string; caption: string; children: Child }): Markup {
  return (
    <section class="absolute left-12" style={`top:${props.top}px`}>
      <h2 class="text-lg leading-none font-semibold">{props.title}</h2>
      <p class="mt-[11px] text-xs leading-none text-muted">{props.caption}</p>
      <div class="mt-[12px]">{props.children}</div>
    </section>
  );
}

function Row(props: { children: Child }): Markup {
  return <div class="flex items-start gap-10">{props.children}</div>;
}

/// A labelled view with an optional note under it.
function View(props: { label: string; note?: string; children: Child }): Markup {
  return (
    <figure class="w-[280px] shrink-0">
      <figcaption class="text-xs leading-none font-semibold text-soft">{props.label}</figcaption>
      <div class="mt-[18px]">{props.children}</div>
      {props.note === undefined ? null : <p class="mt-[14px] text-xs leading-none text-[#8a8a93]">{props.note}</p>}
    </figure>
  );
}

/// Shows only part of a panel, with a rounded outline. The panel is square at the crop's edges.
function Cropped(props: { crop: Crop; children: Child }): Markup {
  return (
    <div class="relative w-[280px] overflow-hidden" style={`height:${props.crop.height}px`}>
      <div style={`margin-top:-${props.crop.top}px`}>{props.children}</div>
      <div class="absolute inset-0 rounded-[10px] border border-border" />
    </div>
  );
}
