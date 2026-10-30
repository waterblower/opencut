import { COLOR } from "../theme.ts";
import type { Markup } from "@design/jsx/jsx-runtime";

/// The sample 1920×1080 frame used as video content in mock-ups: a dusk city street with a
/// silhouette in front. It scales to `width`; the height follows the 16:9 ratio.
/// The gradient ids are shared, so every scene on a page reuses the first definitions.
export function Scene(props: { width: number; class?: string }): Markup {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class={props.class}
      width={props.width}
      height={props.width * 9 / 16}
      viewBox="0 0 1920 1080"
    >
      <defs>
        <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#1b2745" />
          <stop offset="0.55" stop-color="#7a4a62" />
          <stop offset="1" stop-color="#e2905c" />
        </linearGradient>
        <radialGradient id="sun">
          <stop offset="0" stop-color="#ffe2a8" />
          <stop offset="0.45" stop-color="#ffc77a" stop-opacity="0.9" />
          <stop offset="1" stop-color="#ffb066" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="street" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#3a4256" />
          <stop offset="0.6" stop-color="#8a5a4c" />
          <stop offset="1" stop-color="#d48a56" />
        </linearGradient>
      </defs>
      <rect width="1920" height="1080" fill="url(#sky)" />
      <circle cx="1340" cy="610" r="300" fill="url(#sun)" />
      <path
        d="M0 640 C 260 560 480 600 700 560 S 1160 520 1400 590 S 1760 560 1920 600 L1920 1080 L0 1080Z"
        fill="#4a3e5e"
      />
      <rect x="40" y="470" width="150" height="610" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="58" y="500" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="160" y="500" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="126" y="546" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="92" y="592" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="160" y="592" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="58" y="638" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="126" y="638" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="92" y="684" width="14" height="20" opacity="0.8" />
      <rect x="200" y="410" width="120" height="670" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="218" y="440" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="286" y="486" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="252" y="532" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="218" y="578" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="286" y="578" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="252" y="624" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="218" y="670" width="14" height="20" opacity="0.8" />
      <rect x="330" y="520" width="170" height="560" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="348" y="550" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="450" y="550" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="416" y="596" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="382" y="642" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="450" y="642" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="348" y="688" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="416" y="688" width="14" height="20" opacity="0.8" />
      <rect x="510" y="360" width="110" height="720" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="528" y="390" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="596" y="436" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="562" y="482" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="528" y="528" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="596" y="528" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="562" y="574" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="528" y="620" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="596" y="666" width="14" height="20" opacity="0.8" />
      <rect x="630" y="450" width="160" height="630" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="648" y="480" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="750" y="480" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="716" y="526" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="682" y="572" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="750" y="572" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="648" y="618" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="716" y="618" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="682" y="664" width="14" height="20" opacity="0.8" />
      <rect x="800" y="300" width="90" height="780" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="818" y="330" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="852" y="422" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="818" y="468" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="852" y="514" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="818" y="560" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="852" y="652" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="818" y="698" width="14" height="20" opacity="0.8" />
      <rect x="900" y="420" width="180" height="660" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="918" y="450" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1020" y="450" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="986" y="496" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1054" y="496" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="952" y="542" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1020" y="542" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="918" y="588" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="986" y="588" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="952" y="634" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1054" y="634" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="918" y="680" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1020" y="680" width="14" height="20" opacity="0.8" />
      <rect x="1090" y="500" width="140" height="580" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1108" y="530" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1176" y="576" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1142" y="622" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1108" y="668" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1176" y="668" width="14" height="20" opacity="0.8" />
      <rect x="1240" y="560" width="120" height="520" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1258" y="590" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1326" y="636" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1292" y="682" width="14" height="20" opacity="0.8" />
      <rect x="1370" y="470" width="150" height="610" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1388" y="500" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1490" y="500" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1456" y="546" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1422" y="592" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1490" y="592" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1388" y="638" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1456" y="638" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1422" y="684" width="14" height="20" opacity="0.8" />
      <rect x="1530" y="380" width="110" height="700" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1548" y="410" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1616" y="456" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1582" y="502" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1548" y="548" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1616" y="548" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1582" y="594" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1548" y="640" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1616" y="686" width="14" height="20" opacity="0.8" />
      <rect x="1650" y="500" width="170" height="580" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1668" y="530" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1770" y="530" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1736" y="576" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1702" y="622" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1770" y="622" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1668" y="668" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1736" y="668" width="14" height="20" opacity="0.8" />
      <rect x="1830" y="430" width="120" height="650" fill="#251f33" />
      <rect fill={COLOR.accentSoft} x="1848" y="460" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1916" y="506" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1882" y="552" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1848" y="598" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1916" y="598" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1882" y="644" width="14" height="20" opacity="0.8" />
      <rect fill={COLOR.accentSoft} x="1848" y="690" width="14" height="20" opacity="0.8" />
      <path d="M0 820 L1920 760 L1920 1080 L0 1080Z" fill="url(#street)" />
      <path
        d="M0 900 L1920 840"
        stroke="#f2c38a"
        stroke-width="5"
        stroke-dasharray="60 50"
        opacity="0.5"
      />
      <path d="M150 700 l300 -20 l20 70 l-340 20Z" fill="#c4553f" />
      <rect x="160" y="770" width="12" height="150" fill="#1d1824" />
      <rect x="450" y="750" width="12" height="150" fill="#1d1824" />
      <path d="M570 700 l300 -20 l20 70 l-340 20Z" fill="#e0a24a" />
      <rect x="580" y="770" width="12" height="150" fill="#1d1824" />
      <rect x="870" y="750" width="12" height="150" fill="#1d1824" />
      <path d="M990 700 l300 -20 l20 70 l-340 20Z" fill="#3f7f86" />
      <rect x="1000" y="770" width="12" height="150" fill="#1d1824" />
      <rect x="1290" y="750" width="12" height="150" fill="#1d1824" />
      <path d="M1410 700 l300 -20 l20 70 l-340 20Z" fill="#c4553f" />
      <rect x="1420" y="770" width="12" height="150" fill="#1d1824" />
      <rect x="1710" y="750" width="12" height="150" fill="#1d1824" />
      <circle cx="960" cy="560" r="78" fill="#1a1520" />
      <path d="M800 1080 C 800 760 860 660 960 660 C 1060 660 1120 760 1120 1080Z" fill="#1a1520" />
    </svg>
  );
}
