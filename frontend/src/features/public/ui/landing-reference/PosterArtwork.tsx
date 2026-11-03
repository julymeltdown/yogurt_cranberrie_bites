/** Newly authored vector lettering and non-character ornaments, interpreted from
 * the user's d9a8fae2 reference. No screenshot pixels, fonts or character art. */
const letters: Record<string, string> = {
  M: 'M1 68 5 13 9 3 14 17 19 33 27 5 33 2 32 22 37 70 29 65 26 38 20 54 16 45 12 29 9 71 7 61 4 74Z',
  Y: 'M0 5 8 0 19 25 29 2 37 6 24 39 21 71 13 64 12 40Z',
  C: 'M36 9 24 0 9 5 3 15 0 35 3 57 12 68 28 67 37 54 31 48 24 58 15 55 10 44 11 25 16 14 26 13 31 21 33 17Z',
  H: 'M1 4 10 0 11 28 26 25 29 2 37 7 34 69 26 63 25 38 11 40 8 70 0 64Z',
  E: 'M0 8 34 0 37 10 11 19 12 30 30 25 33 37 11 42 12 54 36 49 34 64 1 70 3 36Z',
  I: 'M8 2 23 0 19 19 21 62 14 71 9 60 11 22Z',
  A: 'M0 68 12 5 23 0 38 65 28 72 24 48 13 51 9 66ZM16 36 22 34 19 20Z',
  L: 'M1 5 12 0 10 54 36 49 34 65 0 70 3 26Z',
  R: 'M0 6 22 0 34 10 37 20 32 34 24 40 40 68 31 66 17 47 12 42 9 71 5 64 1 75 2 35ZM11 15 10 32 22 29 27 21 23 12Z',
  O: 'M2 17 13 1 28 0 38 17 36 54 24 70 8 66 0 47ZM13 17 9 34 11 49 18 59 26 51 29 33 26 17 21 10Z',
  N: 'M0 66 2 8 10 0 26 40 26 7 36 0 35 67 27 71 11 31 10 62Z',
}

export function PosterWordmark() {
  return <svg className="landing-wordmark" viewBox="0 0 430 225" aria-hidden="true" focusable="false">
    <g fill="currentColor" fillRule="evenodd" stroke="#161413" strokeWidth="1.2" transform="translate(15 35) rotate(-4 200 50) skewX(-5)">
      {[...'MYCHEMICAL'].map((letter, index) => <path key={index} d={letters[letter]} transform={`translate(${index * 39} ${[5, 1, -5, 0, -7, -2, -4, 0, -5, -2][index]}) rotate(${index % 3 === 0 ? -3 : 2} 19 35)`} />)}
    </g>
    <g fill="currentColor" fillRule="evenodd" stroke="#161413" strokeWidth="1.2" transform="translate(48 111) rotate(-3 170 40) scale(1.28 1.2)">
      {[...'ROMANCE'].map((letter, index) => <path key={index} d={letters[letter]} transform={`translate(${index * 39} ${[0, 3, -3, 1, -2, 2, -2][index]}) rotate(${index % 2 ? 4 : -4} 19 35)`} />)}
    </g>
    <g fill="currentColor"><path d="m37 59-4-26 11 22M81 71l5-39 3 44M154 55l4-29 5 31M269 46l6-27 1 29M357 66l11-22-5 27M108 173l4 42 6-34M169 181l8 36 2-37M334 174l6 30 4-33" />
      <path d="m20 94 9-3 4 3-8 2M404 76l12-3 5 3-14 2M58 185l-13 10 21-6M362 192l22 3-12-7" /></g>
    <g fill="none" stroke="#161413" strokeWidth="1.3"><path d="m27 62 7 18m27-27 6 12m20 19 4 18m23-26 5-17m22-6 2 11m32 12-4 14m33-26 2 12m16-22 7 18m36-25-5 13m27 4 5 11m31-25-2 14m32 1 3 13M79 141l4 17m22 13 9-12m31-17 5 12m20 31-2 13m34-35 8-12m22-2 4 18m31-20-3 13m23 1 6 14m25-8 6 10m19-35 3 10" /></g>
  </svg>
}

export function PosterWeb() {
  return <svg className="landing-web" viewBox="0 0 160 160" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth="1.1">
    <path d="M0 0 158 29M0 0l121 79M0 0l79 125M0 0l31 159M0 0v151M0 0h154" />
    <path d="M0 23Q13 14 24 5M0 48q11-8 10-25 16 0 22-15M0 78q24-15 16-39 26 0 36-29M0 111q34-17 24-55 32 2 49-42M0 143q43-22 31-70 45 2 65-55M31 73q24 4 31-32 32 6 37-22M24 56q18 1 23-25 24 6 26-17M16 39q14 0 16-17 16 6 20-12M40 105q23-20 38-25 4-29 43-34M47 139q26-24 50-32 7-39 52-50" />
  </svg>
}

export function PosterScene() {
  return <svg className="landing-scene" viewBox="0 0 360 240" aria-hidden="true" focusable="false">
    <defs><pattern id="landing-moon-grain" width="8" height="9" patternUnits="userSpaceOnUse"><path d="m1 2 2 1m2 3h1" stroke="#5e5850" strokeWidth=".6" /></pattern></defs>
    <circle cx="269" cy="109" r="69" fill="#dcd2be" />
    <path d="m228 64 14-11 5 8-6 14-14 2-6-7m51-24 15 7 4 11-9 5-13-10m31 16 21 7 5 22-12 4-13-16m-82 22 13-8 8 5-6 15-16 2m35-22 8-9 12 5 1 13-15 3m-11 22 19 4 7 18-10 17-15-4-7-16m48-16 14-5 8 9-4 17-13 3m-63 30 7 10-5 9-12-4" fill="#8f877a" opacity=".65" />
    <circle cx="269" cy="109" r="68" fill="url(#landing-moon-grain)" />
    <g transform="translate(45 38)">
      <path d="M-3 62q10 46-23 71m17-28q19-3 23-20-18-1-23 20m-6 12q-16-6-15-22 13 2 15 22" fill="#74704b" stroke="#74704b" strokeWidth="2" />
      <path d="M-7 1q6-9 16-3l9-3 8 10q13-3 15 9l8 7-5 11 2 10q-4 12-17 11l-8 10-12-4q-12 6-19-3l-13-3-2-11q-11-6-4-18l-2-9 11-6Z" fill="#ad2d35" />
      <g fill="none" stroke="#241817" strokeWidth="3.2" strokeLinejoin="miter"><path d="M-7 7q14-8 28 2l3 11q17 5 12 20-6 12-20 9-14 12-27-1-11-9-5-22M-5 12q-9 13 0 24 11 13 24 2 13-3 7-16L14 11l-12 8-1 11 12 2 5-9m-5 17 8 11m-27-15-10 9M0 18l-4-11m29 14 12-7" /></g>
    </g>
    <g fill="#161413" stroke="#dfd3bd" strokeWidth="1.15" strokeLinejoin="miter">
      <path d="M12 207v-20h15v-25h7v-18l5-9 5 9v18h11v33h6v-54h8v-12l6-12 6 12v12h8v49h8v-33h7v-31l9-20 9 20v31h7v42h9v-51h8v-20h5v-25l7-14 7 14v25h5v20h8v39h9v-20h7v-21h7v-19l6-9 6 9v19h7v21h7v20h12v-60h9v-14l7-16 7 16v14h10v46h8v-15h9v-19h5v-12l6-12 6 12v12h5v19h9v25h23v26H12Z" />
      <path d="M99 138v-24m-7 11h15m45 10V90m-8 17h16m63 42v-40m-9 13h19m54 42v-27m-7 10h14" />
    </g>
    <path d="M0 211q72-10 133 0t227 1v28H0Z" fill="#161413" />
    <g fill="#e1d7c3"><path d="M64 168h3v6h-3m5 2h3v5h-3m37-27h3v7h-3m46-1h3v8h-3m6 8h3v7h-3m70-3h3v7h-3m66 0h3v6h-3m-3 9h3v5h-3" />
      <path d="m183 40 1 7 5 1-5 2-1 7-2-7-5-2 5-1m-56 19 1 6 4 1-4 2-1 6-1-6-4-2 4-1m-51 43 1 6 5 1-5 2-1 6-1-6-4-2 4-1M335 55l1 5 4 1-4 1-1 5-1-5-4-1 4-1" /></g>
  </svg>
}
