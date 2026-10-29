import {
  Dialog,
  DialogButton,
  DialogButtons,
  DialogField,
  DialogHeader,
  DialogOption,
  DialogTextButton,
} from "../components/dialog.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const FRAME_RATES = ["23.976 fps", "24 fps", "25 fps", "29.97 fps", "30 fps", "50 fps", "59.94 fps", "60 fps"];

export default function DialogsPage(): Markup {
  return (
    <main class="relative h-[922px] w-[1080px] bg-[#f4f4f6] font-sans">
      <At left={40} top={40}>
        <Dialog width={480} title="New timeline">
          <p class="mt-[23px] text-sm leading-none text-muted">In timelines · saved as .timeline (JSON)</p>
          <DialogField class="mt-[19px]" value="final-cut" />
          <DialogButtons class="mt-4">
            <DialogButton label="Cancel" />
            <DialogButton label="Create" primary />
          </DialogButtons>
        </Dialog>
      </At>
      <At left={560} top={40}>
        <Dialog width={480} title="Rename broll-street.mov">
          <DialogField class="mt-5" value="broll-old-town.mov" />
          <DialogButtons class="mt-4">
            <DialogButton label="Cancel" />
            <DialogButton label="Rename" primary />
          </DialogButtons>
        </Dialog>
      </At>
      <At left={40} top={308}>
        <section
          role="dialog"
          class="w-[460px] rounded-xl border border-border bg-panel pb-[19px] text-text shadow-menu"
        >
          <DialogHeader title="Timeline Settings" />
          <div class="px-[19px]">
            <p class="mt-[22px] text-xs leading-none font-semibold text-muted">TIMELINE FRAME RATE</p>
            <div class="mt-[18px] grid grid-cols-2 gap-2">
              {FRAME_RATES.map((rate) => <DialogOption label={rate} selected={rate === "30 fps"} />)}
            </div>
            <p class="mt-4 text-xs leading-4 text-muted">
              Existing edit points keep their elapsed time and snap to the nearest frame in the new rate.
            </p>
          </div>
        </section>
      </At>
      <At left={560} top={308}>
        <Dialog width={460} title="Settings" plainTitle>
          <p class="mt-[22px] text-sm leading-none">MiniMax API key</p>
          <DialogField class="mt-[19px]" value="•••••••••••••••••••••••" selected />
          <p class="mt-4 text-xs leading-4 text-muted">
            Saved in global settings for all projects. Used to send audio to MiniMax for SRT generation.
          </p>
          <DialogButtons class="mt-4">
            <DialogTextButton label="Cancel" />
            <DialogTextButton label="Save" />
          </DialogButtons>
        </Dialog>
      </At>
      <At left={40} top={746}>
        <Dialog width={480} title="Move 测试.timeline to Trash?">
          <DialogButtons class="mt-[34px]">
            <DialogButton label="Cancel" />
            <DialogButton label="Move to Trash" primary />
          </DialogButtons>
        </Dialog>
      </At>
    </main>
  );
}

function At(props: { left: number; top: number; children: Child }): Markup {
  return <div class="absolute" style={`left:${props.left}px;top:${props.top}px`}>{props.children}</div>;
}
