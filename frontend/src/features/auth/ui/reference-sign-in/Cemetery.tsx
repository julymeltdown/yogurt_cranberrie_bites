/** Newly drawn cemetery print: no sampled pixels, character art, or reference-image dependency. */
export function Cemetery({ edge }: { edge: 'top' | 'bottom' }) {
  const top = edge === 'top', paper = '#eee5d6'
  return <svg className={`auth-cemetery auth-cemetery--${edge}`} viewBox="0 0 320 120" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    {top ? <>
      <path fill="currentColor" d="M0 0H320V120L316 109 312 112 310 100 306 102 304 90 299 93 294 83 289 85 285 77 280 79 274 72 267 75 261 68 253 72 247 65 238 69 231 63 222 66 212 61 201 64 191 60 180 63 170 59 159 63 148 60 136 65 125 62 114 67 105 65 95 70 87 68 79 74 72 72 64 79 58 77 52 85 46 82 41 92 35 90 30 101 25 99 20 110 15 108 10 119 5 116 0 120Z" />
      <g fill="none" stroke={paper} strokeWidth="1.7">
        <path d="M42 8v26M34 17h16M92 9h7v13h11v7H99v26h-7V29H81v-7h11ZM159 12v33M148 23h22M209 34v23M201 42h16" />
        <path d="m264 13 10 35-28-21 35 1-28 22 11-37 17 15-35-1 28 21M91 11h5v14h11M96 30v23" strokeWidth="1.1" />
        <path d="M25 64v-8a4 4 0 0 1 8 0v8M63 54V44h7v10M119 52v-8a5 5 0 0 1 10 0v8M180 53V42h7v11M225 62v-9a5 5 0 0 1 10 0v9" />
      </g>
      <g fill={paper}>
        {[18, 48, 77, 110, 144, 170, 197, 241, 266].map((x, i) => <path key={x} d={`M${x} ${73 - i % 4 * 5}v-${15 + i % 3 * 4}h-3v-3h3v-6h3v6h4v3h-4v${15 + i % 3 * 4}Z`} />)}
        {Array.from({ length: 74 }, (_, i) => <rect key={i} x={(i * 47 + 13) % 319} y={15 + (i * 31) % 80} width={i % 5 === 0 ? 2 : 1} height={i % 3 + 1} opacity={i % 4 === 0 ? .45 : .85} />)}
      </g>
      <g fill="none" stroke="currentColor" strokeWidth="1.4"><path d="m263 61 57 27m-56-20 56 28m-52-37 2 36m12-30 3 43m10-37 5 46m8-42 7 44M260 68l12-12 13 14 10-3 10 15 10-2" /></g>
      <g fill="currentColor">{Array.from({ length: 47 }, (_, i) => <rect key={i} x={(i * 37) % 320} y={61 + Math.abs(160 - (i * 37) % 320) * .22 + i % 4 * 3} width={.6 + i % 3 * .5} height={.8 + i % 4 * .5} />)}</g>
    </> : <>
      <path fill="currentColor" d="M0 120H320V102l-10 2-4-5-11 5-5-6-7 5-9-4-6 5-9-4-6 6-8-4-10 3-8-6-8 5-7-3-10 7-10-4-8 3-7-4-8 5-7-4-10 5-8-4-9 5-10-4-9 4-7-6-10 4-9-4-8 4-8-5-10 4-8-4-7 4-7-6L0 108Z" />
      <g fill="currentColor">
        <path d="m20 12 7 2-7 35 16 3-2 7-16-3-9 49-7-1 9-50-13-2 1-7 14 3ZM302 36l7-2 5 19 6-2 1 7-6 2 9 42-7 2-9-43-12 3-2-7 12-3Z" />
        {Array.from({ length: 26 }, (_, i) => { const x = 14 + i * 12, y = 83 + i % 4 * 5; return <path key={i} d={`M${x} 114V${y + 7}h-3v-3h3v-6h3v6h4v3h-4v${114 - y - 7}Z`} /> })}
        {[37, 81, 121, 173, 217, 269].map((x, i) => <path key={x} d={`M${x} 115V${97 - i % 3 * 5}a4 4 0 0 1 8 0v${18 + i % 3 * 5}Z`} />)}
        {Array.from({ length: 61 }, (_, i) => <rect key={i} x={(i * 41) % 319} y={87 + i * 17 % 30} width={i % 3 + 1} height={i % 4 + 1} />)}
      </g>
      <g fill={paper}>{Array.from({ length: 32 }, (_, i) => <rect key={i} x={(i * 29) % 320} y={106 + i % 4 * 3} width="1" height="2" />)}</g>
    </>}
  </svg>
}
