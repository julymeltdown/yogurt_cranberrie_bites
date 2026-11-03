import { useId } from 'react'

/** Newly drawn print ornaments and lettering. No target pixels, downloaded logo,
 * external font, character design or venue/wayfinding information is used. */
const letters: Record<string, string> = {
  M: 'M0 83 3 18 5 3 11 18 20 40 29 9 34 0 35 22 40 73 34 64 30 39 22 62 18 50 10 31 8 70 6 64 4 87Z',
  Y: 'M0 0 10 8 18 30 29 1 34 0 24 43 20 84 17 69 13 75 13 43Z',
  C: 'M37 5 26 1 11 8 4 18 0 42 4 63 12 73 24 70 37 56 29 59 20 62 12 54 9 39 14 22 27 15 33 19 35 13Z',
  H: 'M1 9 10 0 11 29 26 24 28 5 31 0 34 13 32 74 27 65 25 37 11 41 8 78 5 65 0 72Z',
  E: 'M0 12 32 2 38 0 33 12 11 22 12 34 31 28 30 40 12 45 11 57 36 51 34 62 1 77 3 43Z',
  I: 'M9 3 20 0 17 23 20 62 15 79 13 65 8 69 11 24Z',
  A: 'M0 80 12 16 15 2 21 0 27 20 38 69 31 65 29 78 23 48 12 54 7 72ZM15 40 22 37 19 23Z',
  L: 'M2 8 13 0 11 59 34 51 40 51 32 66 0 79 3 30Z',
  R: 'M1 12 17 2 28 0 38 10 39 24 32 38 23 45 41 80 31 70 19 53 13 47 12 76 8 69 4 91 3 43ZM13 19 12 36 24 30 29 22 25 13Z',
  O: 'M3 20 15 3 26 0 37 11 39 32 32 59 22 74 9 72 0 53ZM15 21 10 39 13 56 20 60 28 47 30 28 25 14 21 14Z',
  N: 'M0 77 3 16 9 0 28 44 28 9 34 0 36 19 34 72 28 79 12 37 9 68 5 65Z',
}
const firstLine = [
  ['M', 9, 28, .99, .87, -6], ['Y', 46, 22, .91, .86, 5],
  ['C', 77, 21, 1.02, .90, -4], ['H', 112, 13, 1.04, 1.02, 0],
  ['E', 149, 18, 1.02, .91, -3], ['M', 183, 5, 1.02, 1.08, 1],
  ['I', 223, 10, .91, 1.04, -6], ['C', 244, 9, .98, 1.03, 2],
  ['A', 280, 0, 1.01, 1.12, -3], ['L', 317, 2, 1.01, 1.06, 2],
] as const
const secondLine = [
  ['R', 59, 85, 1.72, 1.29, -11], ['O', 116, 87, 1.39, 1.08, 14],
  ['M', 158, 84, 1.20, 1.10, -3], ['A', 203, 83, 1.12, 1.05, 1],
  ['N', 241, 81, 1.12, 1.02, -4], ['C', 280, 81, 1.10, 1.04, 4],
  ['E', 319, 78, 1.16, 1.04, -2],
] as const

export function PosterWordmark() {
  const ink = useId()
  return <svg className="landing-wordmark" viewBox="0 0 380 222" aria-hidden="true" focusable="false">
    <defs><mask id={ink} maskUnits="userSpaceOnUse" x="-10" y="-10" width="410" height="250">
      <path d="M-10-10h410v250H-10Z" fill="white" />
      <g fill="none" stroke="black" strokeWidth=".65">
        <path d="m21 31-1 25 2 8m8-9 1 9m29-30 1 11m22-9-3 14m16-23-3 9m24-7 1 21m16-18 2 9m21-26-2 30m15-29 1 8m19-33-2 30m9-12 2 18m22-37-2 24m28-20 1 20m15-32 3 21m16-13-2 28m22-36 2 22m13-15-2 27m23-28 2 33m-246 47 5 22m9-27-4 26m24-24 1 19m11-31 4 21m24-25-2 28m14-12 4 16m28-36-3 27m21-27 2 27m17-16 2 17m21-26-2 18m14-25-1 22m19-21 3 13m15-19-2 25" />
        <path d="m11 72 39-12m55 2 34-9m42 7 31-5m38-3 21-9m40-4 30-7M71 139l19-8m58 1 29-9m36 13 17-7m41-1 28-11m15 11 29-8" />
      </g>
    </mask></defs>
    <g fill="currentColor" fillRule="evenodd" mask={`url(#${ink})`} transform="translate(3 15) rotate(-4 186 94)">
      {[...firstLine, ...secondLine].map(([letter, x, y, sx, sy, angle], index) => <path key={index} d={letters[letter]} stroke="#161413" strokeWidth=".8" transform={`translate(${x} ${index < firstLine.length ? y : y + 10}) rotate(${angle} 19 36) scale(${sx} ${sy})`} />)}
      <path d="m19 69-3 31 5-17 1 24 4-34m17-39-5-20 9 18m65-7 1-22 3 20m22 37 1 28 3-28m35-28 3-30 2 31m24 28 3 30 2-32m27-11 1-50 4 49m59-10 3-43 3 40m19 2 5-33 2 33m30-13 4-36 1 40m22-1 8-23-1 26M64 126l-17 11 20-6m6 9-21 17 22-7m11 10 16 53-5-60m41 1-1 27 5-23m35-5 5 24 2-24m24-7 5 24 1-28m25-2 3 32 4-28m26-12 7 24 1-26m25 0 6 20 1-22m28-6 4 19 2-21m16-16 28-8-26 1m-3 29 30-5-26-3" />
    </g>
  </svg>
}

export function PosterWeb() {
  return <svg className="landing-web" viewBox="0 0 160 160" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth=".9">
    <path d="M0 0 158 29M0 0l121 79M0 0l79 125M0 0l31 159M0 0v151M0 0h154" />
    <path d="M0 23Q13 14 24 5M0 48q11-8 10-25 16 0 22-15M0 78q24-15 16-39 26 0 36-29M0 111q34-17 24-55 32 2 49-42M0 143q43-22 31-70 45 2 65-55M31 73q24 4 31-32 32 6 37-22M24 56q18 1 23-25 24 6 26-17M16 39q14 0 16-17 16 6 20-12M40 105q23-20 38-25 4-29 43-34M47 139q26-24 50-32 7-39 52-50M2 5l3 8m8-11 4 4m20 9 1 7m29 6 3 8m24 10 6-2m10 20 5 3M1 121l7-2m18-7 4 6m39-14-4 9" />
  </svg>
}

const craters = [[232, 60, 3], [249, 47, 2], [289, 49, 10], [303, 69, 14], [314, 94, 7], [292, 91, 6], [264, 78, 2], [248, 86, 8], [231, 102, 3], [254, 119, 6], [282, 129, 7], [304, 121, 3], [239, 140, 2], [271, 151, 3], [322, 112, 3]] as const
function craterShape(x: number, y: number, radius: number, seed: number) {
  return Array.from({ length: 14 }, (_, step) => {
    const a = step / 14 * Math.PI * 2
    const r = radius * (.84 + ((step * 17 + seed * 13) % 11) / 50)
    return `${step ? 'L' : 'M'}${(x + Math.cos(a) * r).toFixed(2)} ${(y + Math.sin(a) * r).toFixed(2)}`
  }).join('') + 'Z'
}

export function PosterScene() {
  const moon = useId(), grain = useId(), pitted = useId(), pits = useId()
  return <svg className="landing-scene" viewBox="0 0 360 220" aria-hidden="true" focusable="false">
    <defs>
      <pattern id={pits} width="7" height="9" patternUnits="userSpaceOnUse"><path d="M0 0h7v9H0Z" fill="white" /><path d="m1 2 1 1m3 3h1m-4 2h1" stroke="black" strokeWidth=".9" /></pattern>
      <mask id={pitted}><path d="M180 25h170v160H180Z" fill={`url(#${pits})`} /></mask>
      <clipPath id={moon}><circle cx="266" cy="103" r="66" /></clipPath>
      <pattern id={grain} width="19" height="23" patternUnits="userSpaceOnUse">
        <path d="m1 4 1 1m6-3v1m5 5 1 1m3 8 1-1m-12 4h1m-6-7 1 1m7 1h1m-3 8h1" stroke="#544c43" strokeWidth=".7" />
        <path d="m3 2h1m8 2h1m4 6v1m-9 8h1m7 4h1" stroke="#faf0d8" strokeWidth=".9" />
      </pattern>
    </defs>
    <g clipPath={`url(#${moon})`}>
      <circle cx="266" cy="103" r="66" fill="#ded4bf" />
      {craters.map(([x, y, r], index) => <g key={index}>
        <path d={craterShape(x + 1, y + 1, r + 1, index)} fill="#eee4cd" />
        <path mask={`url(#${pitted})`} d={craterShape(x, y, r, index)} fill={index % 3 === 0 ? '#726c62' : '#9c9486'} />
        {Array.from({ length: 10 }, (_, n) => <path key={n} d={`m${x - r + n * r / 5} ${y + ((n * 7 + index * 3) % 13) - 6} .6 -.4`} stroke="#524d46" strokeWidth=".7" />)}
      </g>)}
      <g fill="#777064" opacity=".55">{Array.from({ length: 130 }, (_, n) => <circle key={n} cx={205 + (n * 43 % 125)} cy={39 + (n * 31 % 130)} r={.25 + n % 4 * .22} />)}</g>
      <circle cx="266" cy="103" r="66" fill={`url(#${grain})`} />
    </g>
    <g className="landing-rose" transform="translate(44 44)">
      <path d="M-2 29Q1 67-27 100M-8 62q15 0 19-12-12-2-19 12M-14 76q-16-1-18-12 12-1 18 12" fill="#6f7145" stroke="#74734b" strokeWidth="1.7" />
      <path d="M-16-29q6-4 13 1 8-10 17-3 10-3 13 7 13-2 14 11 10 4 3 15 7 9-3 15-1 12-14 11-6 11-17 4-11 7-18-2-13 1-15-10-12-5-7-16-4-11 7-16-1-11 7-7Z" fill="#a92a31" stroke="#211817" strokeWidth="1.5" />
      <g fill="none" stroke="#201716" strokeWidth="2.8" strokeLinejoin="miter">
        <path d="M-15-27q-10 3-8 12l7 6q-13-1-13 11 1 11 14 15l-4 7q9 10 21 2 11 7 21-3l-3-8q15-5 11-16-2-10-14-10l2-10q-7-11-20-5l-6 5q-10-3-13 7-3 10 6 16l-5 7q9 9 19 2l4-8-5-6-8 4 3 6 5 1M-5-15q16-8 25 3 6 10-6 22-9 9-21 1-13-8-9-22m34-7q6 9-3 16m-25-9q-12 1-16-7m-3 22q8 2 12-5m10 25q6-4 2-11m20-5q6 2 9 7m-3-30 9-1m-41 26-8 2" />
        <path d="m-22-22 5 1m-13 18 6 1m12-18 5 1m-5 30 5-2m16 14 2-5m16-13-4-1M2-25l2 6m16-5-1 6" strokeWidth="1" />
      </g>
    </g>
    <g className="landing-cemetery" fill="#161413" stroke="#d8cdb7" strokeWidth="1.05" strokeLinejoin="miter">
      <path d="M8 194v-21h8v-11h9v-6h10v19h12v-29h7v-7h4v-14l3-7 3 7v14h5v7h9v21h13v-13h8v-9h5v-15h6v-11l5-12 5 12v11h6v15h6v27h12v-10h7v-18h7v-11h5v-17h5v-16l4-11 4 11v16h5v17h6v11h7v29h14v-12h8v-9h6v-14h7v-15l4-9 4 9v15h7v14h8v20h12v-12h8v-10h7v-24h7v-13l5-9 5 9v13h7v24h8v23h8v-8h9v-8h6v-13h4v-8l4-8 4 8v8h5v13h8v17h13v-5h8v18Z" />
      <path d="M58 140v-26m-8 8h16m48 9V96m-9 13h18m53 10V83m-9 11h18m48 59v-35m-9 10h18m56 25v-37m-9 12h18m34 44v-23m-7 8h14" />
      <path d="M18 181v-8m33-6v9m68-28v9m52-19v8m14 10v9m44-1v9m61-9v9m33 2v8" strokeWidth="2" />
      <path d="M32 191v-14m-4 4h8m60 11v-16m-4 4h8m103 13v-13m-4 4h8m98 9v-13m-4 4h8" />
    </g>
    <path d="M0 196q20-5 33-1t31 0 28 1 29-2 31 2 29-1 33 2 30-1 30 1 30-3 37 1v25H0Z" fill="#161413" />
    <g fill="#e1d7c3"><path d="m182 37 1 6 4 1-4 1-1 6-1-6-4-1 4-1m-53 32 1 5 4 1-4 1-1 5-1-5-4-1 4-1m-40 29 1 5 4 1-4 1-1 5-1-5-4-1 4-1M338 67l1 4 3 1-3 1-1 4-1-4-3-1 3-1" /></g>
  </svg>
}

/** Worn print perimeter on all four sides, never a left accent rail. */
export function PosterWear() {
  return <svg className="landing-print-wear" viewBox="0 0 360 600" preserveAspectRatio="none" aria-hidden="true" focusable="false" fill="none" stroke="currentColor">
    <path d="m4 2 96 1 73-1 88 1 95-1M3 4l1 106-1 81 1 72-1 96 1 103-1 91 1 43m353-591-1 112 1 107-1 83 1 91-1 84 1 106M3 597l75-1 80 1 113-1 85 1" strokeWidth="1.1" />
    <path d="m12 3 8 2m17-3 4 2m17 1 13-2m26 1 9-1m28 2 8-1m24-2 14 2m52-1 4 2m31-2 14-1m38 2 7-1m22 2 8-2M3 40l2 4m-2 48 2 6m-1 75 2 5m-2 112 2-8m-2 63 2 7m-2 79 2-4m-1 112 2 6m-2 45 8 3m343-550-2 5m2 61-2 8m2 55-1 7m1 86-2 4m2 94-2 10m2 76-2 11m2 89-2 8M16 594l7 2m23-1 14 2m29-3 5 2m19-1 11-1m21 1 6 2m37-2 5 2m31-2 12 1m20-2 15 2m23 0 5-2m30 1 10 2" strokeWidth="2.1" />
    <g strokeWidth=".65">{Array.from({ length: 90 }, (_, n) => <path key={n} d={`m${11 + (n * 71 % 338)} ${12 + (n * 137 % 577)} ${n % 3 === 0 ? '2 -1' : '.5 1'}`} />)}</g>
    <path d="m11 385 7-2 2 1m-5 13 6 1m306-33 8-2m-3 9 6-1M12 552l9 2m6-8 4-2m297 0 7-1m-3 6 7 1m-201-61 7-1m45 5 3-1" strokeWidth=".8" />
  </svg>
}
