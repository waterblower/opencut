import { Switch } from "../components/panel.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";
export default function SwitchMotionPage(): Markup {
  return <main class="w-[900px] bg-sheet p-12 font-sans text-ink">
    <h1 class="text-2xl font-bold">Switch · 切换动画</h1>
    <p class="mt-3 text-sm text-dim">180 ms · ease-out · 圆点滑动，轨道与圆点颜色同步过渡，无弹跳。</p>
    <section class="mt-8 border-t border-rule pt-6">
      <h2 class="text-lg font-semibold">关闭 → 开启</h2>
      <div class="mt-4 grid grid-cols-3 gap-6">
        {[0, 0.5, 1].map((progress) => <div>
          <p class="mb-3 text-sm text-dim">{progress === 0 ? "起点" : progress === 1 ? "终点 · 180 ms" : "过渡示意 · 位移 50%"}</p>
          <div class="flex h-24 items-center justify-center rounded-lg bg-panel">
            {progress !== 0.5 ? <Switch on={progress === 1} /> : <span class="relative h-5 w-[34px] rounded-full" style="background:color-mix(in srgb,var(--color-border),var(--color-accent) 50%)"><span class="absolute top-[3px] left-[10px] size-3.5 rounded-full" style="background:color-mix(in srgb,var(--color-muted),var(--color-panel) 50%)" /></span>}
          </div>
        </div>)}
      </div>
      <p class="mt-5 text-sm leading-6 text-dim">关闭时反向过渡。连续点击从当前视觉位置转向新状态；首次显示与切换选中对象时直接呈现当前状态。点击立即更新功能状态；锁定时不响应切换。</p>
    </section>
  </main>;
}
