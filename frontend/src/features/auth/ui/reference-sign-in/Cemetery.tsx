/** Newly authored scenery, never sampled from the reference or character pack. */
export function Cemetery({ edge }: { edge: 'top' | 'bottom' }) {
  return <svg className={`auth-cemetery auth-cemetery--${edge}`} viewBox="0 0 320 100" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <path fill="currentColor" d={edge === 'top'
      ? 'M0 0H320V99l-6-6-2-11-6 3-3-11-7 4-4-10Q160 37 27 77l-4-5-4 12-6-3-3 13-6-3L0 100Z'
      : 'M0 100H320V67l-17 7-10-11-19 10-14-9-15 10-19-7-17 10-15-9-18 8-20-10-16 8-21-8-12 10-21-9-12 11-16-8-14 8-13-9L0 77Z'} />
    <g fill="none" stroke={edge === 'top' ? '#eee5d6' : 'currentColor'} strokeWidth="3">
      {edge === 'top' ? <><path d="M49 29v29M40 39h18M110 10v46M100 24h20M179 25v30M169 36h20M266 33l4 29M257 43l20-3" strokeWidth="2" /><path d="m295 16 5 16 12-10-6 17 9 12-17-5-11 10 5-18-11-9 16 2Z" strokeWidth="1.3" />
        <path d="M8 70v-9a4 4 0 0 1 8 0v9M29 65V54h7v11M66 64v-8a5 5 0 0 1 10 0v8M87 60V49h6v11M129 58v-9a5 5 0 0 1 10 0v9M153 57V46h6v11M197 60v-8a4 4 0 0 1 8 0v8M216 63V51h8v12M239 68v-9a5 5 0 0 1 10 0v9M284 74V61h7v13M309 79V67h6v12M3 73l313-2M21 59v17M81 56v16M145 53v18M209 55v16M277 61v14" strokeWidth="1.2" /></>
        : <><path d="m27 22-7 59M7 40l36 4M280 19l8 62M266 35l33-5M77 53v24M67 62h20M230 47v30M219 59h23M125 59v20M117 67h17" /><path d="M43 85v-8a8 8 0 0 1 16 0v8M176 87V74a9 9 0 0 1 18 0v13M249 88v-8a7 7 0 0 1 14 0v8" /></>}
    </g>
    <g fill={edge === 'top' ? '#eee5d6' : 'currentColor'}>{[12, 32, 58, 84, 103, 139, 157, 187, 207, 240, 269, 297, 312].map((x, i) => <rect key={x} x={x} y={edge === 'top' ? 56 + i % 4 * 3 : 74 - i % 3 * 3} width={3 + i % 3} height="18" />)}</g>
  </svg>
}
