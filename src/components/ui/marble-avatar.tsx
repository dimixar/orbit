import { twMerge } from 'tailwind-merge'

/**
 * Deterministic generated avatar in the spirit of avvvatars' `marble` style
 * (https://github.com/nusu/avvvatars): a two-hue linear gradient with a soft
 * marble highlight, derived entirely from the seed string. Zero dependencies,
 * works offline — the same seed always renders the same identity.
 */

// Tuned two-stop pairs on the primary hue wheel (cyan/teal family) so every
// result stays on-brand and light/dark safe. Picked to keep 4.5:1 contrast
// against white initials only if initials are added later.
const PALETTES: [string, string][] = [
  ['oklch(0.72 0.14 186)', 'oklch(0.45 0.10 215)'],
  ['oklch(0.75 0.13 170)', 'oklch(0.48 0.11 195)'],
  ['oklch(0.70 0.15 205)', 'oklch(0.42 0.09 245)'],
  ['oklch(0.78 0.11 155)', 'oklch(0.50 0.12 180)'],
  ['oklch(0.68 0.16 220)', 'oklch(0.40 0.10 265)'],
  ['oklch(0.74 0.12 195)', 'oklch(0.44 0.13 160)'],
]

/** FNV-1a 32-bit — stable across runs and machines. */
function hash(seed: string): number {
  let h = 0x811c9dc5
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i)
    h = Math.imul(h, 0x01000193)
  }
  return h >>> 0
}

export interface MarbleAvatarProps {
  seed: string
  alt?: string
  className?: string
}

export function MarbleAvatar({ seed, alt = '', className }: MarbleAvatarProps) {
  const h = hash(seed || 'orbit')
  const [from, to] = PALETTES[h % PALETTES.length]
  const angle = 45 + (h % 180) // diagonal gradient, varies by seed
  const id = `ma-${h.toString(36)}`

  return (
    <span
      role={alt ? 'img' : undefined}
      aria-label={alt || undefined}
      aria-hidden={alt ? undefined : true}
      className={twMerge(
        'inline-block size-(--avatar-size,1.5rem) shrink-0 overflow-hidden rounded-full outline-1 -outline-offset-1 outline-fg/10',
        className,
      )}
    >
      <svg className="size-full" viewBox="0 0 100 100" aria-hidden="true">
        <defs>
          <linearGradient
            id={id}
            x1="0"
            y1="0"
            x2="1"
            y2="1"
            gradientTransform={`rotate(${angle} 0.5 0.5)`}
          >
            <stop offset="0%" stopColor={from} />
            <stop offset="100%" stopColor={to} />
          </linearGradient>
        </defs>
        <rect width="100" height="100" fill={`url(#${id})`} />
        {/* marble highlight */}
        <circle cx="32" cy="30" r="38" fill="oklch(1 0 0 / 0.28)" />
        <circle cx="30" cy="28" r="38" fill="oklch(1 0 0 / 0.14)" />
      </svg>
    </span>
  )
}
