import type { ReactNode } from 'react'

const p = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const

function Svg({ children, size = 18 }: { children: ReactNode; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" {...p}>
      {children}
    </svg>
  )
}

export const Icons = {
  home: (
    <Svg>
      <path d="M3 11l9-8 9 8M5 10v10h14V10" />
    </Svg>
  ),
  people: (
    <Svg>
      <circle cx="9" cy="8" r="3.5" />
      <path d="M2.5 20c0-3.5 3-6 6.5-6s6.5 2.5 6.5 6M16 4.5a3.5 3.5 0 010 7M18 14c2 .8 3.5 2.8 3.5 6" />
    </Svg>
  ),
  tree: (
    <Svg>
      <circle cx="12" cy="5" r="2.5" />
      <circle cx="6" cy="19" r="2.5" />
      <circle cx="18" cy="19" r="2.5" />
      <path d="M12 7.5V12M12 12H6v4.5M12 12h6v4.5" />
    </Svg>
  ),
  fan: (
    <Svg>
      <path d="M3 19a9 9 0 0118 0zM12 19V6M5.5 14.5L12 10l6.5 4.5" />
    </Svg>
  ),
  link: (
    <Svg>
      <path d="M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1" />
    </Svg>
  ),
  check: (
    <Svg>
      <path d="M20 6L9 17l-5-5" />
    </Svg>
  ),
  chart: (
    <Svg>
      <path d="M4 20V10M10 20V4M16 20v-7M22 20H2" />
    </Svg>
  ),
  file: (
    <Svg>
      <path d="M14 3H6a1 1 0 00-1 1v16a1 1 0 001 1h12a1 1 0 001-1V8zM14 3v5h5M9 13h6M9 17h6" />
    </Svg>
  ),
  gear: (
    <Svg>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 00.3 1.9l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.9-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 01-4 0v-.1a1.7 1.7 0 00-1-1.5 1.7 1.7 0 00-1.9.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.9 1.7 1.7 0 00-1.5-1H3a2 2 0 010-4h.1a1.7 1.7 0 001.5-1 1.7 1.7 0 00-.3-1.9l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.9.3h0a1.7 1.7 0 001-1.5V3a2 2 0 014 0v.1a1.7 1.7 0 001 1.5h0a1.7 1.7 0 001.9-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.9v0a1.7 1.7 0 001.5 1H21a2 2 0 010 4h-.1a1.7 1.7 0 00-1.5 1z" />
    </Svg>
  ),
  search: (
    <Svg>
      <circle cx="11" cy="11" r="7" />
      <path d="M21 21l-4.3-4.3" />
    </Svg>
  ),
  undo: (
    <Svg>
      <path d="M9 14L4 9l5-5M4 9h10a6 6 0 010 12h-3" />
    </Svg>
  ),
  redo: (
    <Svg>
      <path d="M15 14l5-5-5-5M20 9H10a6 6 0 000 12h3" />
    </Svg>
  ),
  back: (
    <Svg>
      <path d="M15 18l-6-6 6-6" />
    </Svg>
  ),
  forward: (
    <Svg>
      <path d="M9 18l6-6-6-6" />
    </Svg>
  ),
  plus: (
    <Svg>
      <path d="M12 5v14M5 12h14" />
    </Svg>
  ),
  menu: (
    <Svg>
      <path d="M4 6h16M4 12h16M4 18h16" />
    </Svg>
  ),
  star: (
    <Svg>
      <path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1L3.2 9.5l6.1-.9z" />
    </Svg>
  ),
  timeline: (
    <Svg>
      <path d="M4 6h10M4 12h16M4 18h7" />
      <circle cx="17" cy="6" r="2" />
      <circle cx="14" cy="18" r="2" />
    </Svg>
  ),
  cube: (
    <Svg>
      <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5" />
    </Svg>
  ),
  image: (
    <Svg>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <circle cx="9" cy="10" r="1.6" />
      <path d="M21 16l-5-5-8 8" />
    </Svg>
  ),
  map: (
    <Svg>
      <path d="M9 4L3 6.5v13L9 17l6 3 6-2.5v-13L15 7zM9 4v13M15 7v13" />
    </Svg>
  ),
  calendar: (
    <Svg>
      <rect x="3" y="5" width="18" height="16" rx="2" />
      <path d="M3 10h18M8 3v4M16 3v4" />
    </Svg>
  ),
  book: (
    <Svg>
      <path d="M4 5a2 2 0 012-2h13v16H6a2 2 0 00-2 2zM4 21V5M8 7h7" />
    </Svg>
  ),
  help: (
    <Svg>
      <circle cx="12" cy="12" r="9" />
      <path d="M9.5 9a2.5 2.5 0 114 2c-.8.6-1.5 1-1.5 2M12 17h.01" />
    </Svg>
  ),
}
