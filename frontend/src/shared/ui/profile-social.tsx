import type { ComponentPropsWithRef, ReactNode } from 'react'

/* ---------------------------------------------------------------------------
 * WP-10 profile/social primitives.
 *
 * The reusable presentation layer for the new profile, discovery, media, room,
 * identity and avatar screens. Each primitive is presentation only: it owns no
 * data, starts no request, and keeps whatever accessible role its content
 * needs. Markup and behavior of the screens that use them are unchanged; these
 * replace ad-hoc per-feature styling, not per-feature logic.
 *
 * The corresponding CSS lives in shared/styles/parade-ink/profile-social.css.
 * ------------------------------------------------------------------------- */

const join = (...values: readonly (string | false | null | undefined)[]) =>
  values.filter(Boolean).join(' ')

/* --- SectionHeader --------------------------------------------------------
 * A screen or card section title with an optional count, description and
 * trailing action. Renders an <h2> by default; `level` changes the heading
 * rank so the document outline stays correct without visual drift.
 */
export type SectionHeaderProps = {
  title: string
  level?: 2 | 3 | 4
  count?: number | string
  description?: ReactNode
  action?: ReactNode
  id?: string
}

export function SectionHeader({ title, level = 2, count, description, action, id }: SectionHeaderProps) {
  const Heading = `h${level}` as const
  return (
    <header className="pi-section__header">
      <Heading className="pi-section__heading" id={id}>
        {title}
        {count !== undefined && <span className="pi-section__count">{count}</span>}
      </Heading>
      {action && <div className="pi-section__action">{action}</div>}
      {description && <p className="pi-section__description">{description}</p>}
    </header>
  )
}

/** A titled section: a SectionHeader followed by its content, with one rhythm. */
export function Section({ title, level, count, description, action, id, children, className }: {
  title: string
  level?: 2 | 3 | 4
  count?: number | string
  description?: ReactNode
  action?: ReactNode
  id?: string
  children?: ReactNode
  className?: string
}) {
  return (
    <section className={join('pi-section', className)} {...(id ? { 'aria-labelledby': id } : {})}>
      <SectionHeader title={title} {...(level ? { level } : {})} {...(count !== undefined ? { count } : {})}
        {...(description ? { description } : {})} {...(action ? { action } : {})} {...(id ? { id } : {})} />
      {children}
    </section>
  )
}

/* --- Card -----------------------------------------------------------------
 * A distinct object with its own field and symmetric border. Never a wrapper
 * for each item of a list; use List/Row for that.
 */
export type CardProps = Omit<ComponentPropsWithRef<'div'>, 'title'> & {
  title?: ReactNode
  headerAction?: ReactNode
  footer?: ReactNode
  tone?: 'plain' | 'inset'
  flush?: boolean
}

export function Card({ title, headerAction, footer, tone = 'plain', flush = false, className = '', children, ...props }: CardProps) {
  return (
    <div {...props} className={join('pi-card', tone === 'inset' && 'pi-card--inset', flush && 'pi-card--flush', className)}>
      {(title || headerAction) && (
        <div className="pi-card__header">
          {title && <h3 className="pi-card__title">{title}</h3>}
          {headerAction}
        </div>
      )}
      <div className="pi-card__body">{children}</div>
      {footer && <div className="pi-card__footer">{footer}</div>}
    </div>
  )
}

/* --- List / Row -----------------------------------------------------------
 * A dense list. Rows are separated by a symmetric divider; each row is not its
 * own bordered or accented box. This is the reference's Feed/Meet/Giveaway
 * density expressed as a primitive.
 */
export function List({ children, className, label }: { children: ReactNode; className?: string; label?: string }) {
  return <ul className={join('pi-list', className)} {...(label ? { 'aria-label': label } : {})}>{children}</ul>
}

export type RowProps = Omit<ComponentPropsWithRef<'div'>, 'title'> & {
  media?: ReactNode
  title?: ReactNode
  meta?: ReactNode
  actions?: ReactNode
  layout?: 'default' | 'no-media' | 'stacked'
}

export function Row({ media, title, meta, actions, layout = 'default', className = '', children, ...props }: RowProps) {
  return (
    <div {...props} className={join('pi-row', layout === 'no-media' && 'pi-row--no-media',
      layout === 'stacked' && 'pi-row--stacked', className)}>
      {media && <div className="pi-row__media">{media}</div>}
      <div className="pi-row__body">
        {title && <p className="pi-row__title">{title}</p>}
        {meta && <p className="pi-row__meta">{meta}</p>}
        {children && <div className="pi-row__text">{children}</div>}
      </div>
      {actions && <div className="pi-row__actions">{actions}</div>}
    </div>
  )
}

/** A row whose whole surface is one link. The visible focus ring is symmetric. */
export function RowLink({ href, media, title, meta, actions, className = '', children, ...props }: {
  href: string
  media?: ReactNode
  title?: ReactNode
  meta?: ReactNode
  actions?: ReactNode
  children?: ReactNode
  className?: string
} & Omit<ComponentPropsWithRef<'a'>, 'href' | 'title'>) {
  return (
    <a {...props} href={href} className={join('pi-row-link', className)}>
      {media && <div className="pi-row__media">{media}</div>}
      <div className="pi-row__body">
        {title && <p className="pi-row__title">{title}</p>}
        {meta && <p className="pi-row__meta">{meta}</p>}
        {children && <div className="pi-row__text">{children}</div>}
      </div>
      {actions && <div className="pi-row__actions">{actions}</div>}
    </a>
  )
}

/* --- MediaTile ------------------------------------------------------------
 * A square (or 4:3) media cell that reserves its ratio so a grid never jumps.
 * A missing or failed image shows an explicit state; it never substitutes
 * another image.
 */
export type MediaTileProps = {
  src?: string | null
  alt: string
  caption?: ReactNode
  /** Shown inside the tile when `src` is absent. */
  emptyLabel?: string
  ratio?: 'square' | 'wide'
  loading?: 'eager' | 'lazy'
}

export function MediaTile({ src, alt, caption, emptyLabel = 'No image', ratio = 'square', loading = 'lazy' }: MediaTileProps) {
  return (
    <figure className={join('pi-media-tile', ratio === 'wide' && 'pi-media-tile--wide')}>
      {src
        ? <img src={src} alt={alt} loading={loading} decoding="async" />
        : <span className="pi-media-tile__state">{emptyLabel}</span>}
      {caption && <figcaption className="pi-media-tile__caption">{caption}</figcaption>}
    </figure>
  )
}

export function MediaGrid({ children, className, label }: { children: ReactNode; className?: string; label?: string }) {
  return <ul className={join('pi-media-grid', className)} {...(label ? { 'aria-label': label } : {})}>{children}</ul>
}

/* --- AvatarChip -----------------------------------------------------------
 * A small identity mark. The frame is a symmetric square; when no character is
 * available it shows the initial, never a substitute character image.
 */
export type AvatarChipProps = {
  name: string
  meta?: ReactNode
  /** A rendered character (canvas or img). Absent means the initial is shown. */
  media?: ReactNode
  size?: 'sm' | 'lg'
  href?: string
}

export function AvatarChip({ name, meta, media, size = 'sm', href }: AvatarChipProps) {
  const initial = Array.from(name.trim())[0] ?? '?'
  const content = (
    <>
      <span className="pi-avatar-chip__frame" aria-hidden="true">
        {media ?? <span className="pi-avatar-chip__initial">{initial}</span>}
      </span>
      <span className="pi-avatar-chip__text">
        <span className="pi-avatar-chip__name">{name}</span>
        {meta && <span className="pi-avatar-chip__meta">{meta}</span>}
      </span>
    </>
  )
  const className = join('pi-avatar-chip', size === 'lg' && 'pi-avatar-chip--lg', href && 'pi-avatar-chip--link')
  if (href) return <a className={className} href={href}>{content}</a>
  return <span className={className}>{content}</span>
}

/* --- Badge ----------------------------------------------------------------
 * A short status word. The text always carries the meaning, so the tone is
 * redundant information rather than the only signal.
 */
export type BadgeTone = 'neutral' | 'ok' | 'warn' | 'danger' | 'accent' | 'count'

export function Badge({ children, tone = 'neutral', className }: { children: ReactNode; tone?: BadgeTone; className?: string }) {
  return <span className={join('pi-badge', tone !== 'neutral' && `pi-badge--${tone}`, className)}>{children}</span>
}

/* --- EmptyState -----------------------------------------------------------
 * A quiet statement of what is missing and what to do next.
 */
export function EmptyState({ title, children, action, className }: {
  title: string
  children?: ReactNode
  action?: ReactNode
  className?: string
}) {
  return (
    <div className={join('pi-empty', className)}>
      <p className="pi-empty__title">{title}</p>
      {children && <p className="pi-empty__body">{children}</p>}
      {action && <div className="pi-empty__action">{action}</div>}
    </div>
  )
}

/* --- InlineError ----------------------------------------------------------
 * A refusal or failure attached to a region. `role="alert"` by default so an
 * assistive technology announces it; pass `live="polite"` for a non-urgent
 * message.
 */
export function InlineError({ title, children, action, live = 'assertive', className }: {
  title: string
  children?: ReactNode
  action?: ReactNode
  live?: 'assertive' | 'polite'
  className?: string
}) {
  return (
    <div className={join('pi-inline-error', className)} role="alert" aria-live={live}>
      <p className="pi-inline-error__title">{title}</p>
      {children && <p className="pi-inline-error__body">{children}</p>}
      {action && <div className="pi-inline-error__actions">{action}</div>}
    </div>
  )
}

/* --- Screen ---------------------------------------------------------------
 * The shared page frame: a bounded column with one screen title and consistent
 * vertical rhythm, replacing per-feature padding/max-width/colour rules.
 */
export function Screen({ title, lead, actions, children, className, id }: {
  title: string
  lead?: ReactNode
  actions?: ReactNode
  children?: ReactNode
  className?: string
  id?: string
}) {
  return (
    <div className={join('pi-screen', className)}>
      <header className="pi-screen__header">
        <h1 className="pi-screen__title" {...(id ? { id } : {})}>{title}</h1>
        {lead && <p className="pi-screen__lead">{lead}</p>}
        {actions && <div className="pi-screen__actions">{actions}</div>}
      </header>
      {children}
    </div>
  )
}

/* --- TabStrip -------------------------------------------------------------
 * The compact tab row: equal cells and a black selected state. It renders the
 * same `role="tab"`/`aria-selected` contract the existing feature tab lists
 * already use, so a screen can adopt the presentation without changing its
 * selection logic.
 */
export type TabStripItem<T extends string> = {
  readonly id: T
  readonly label: ReactNode
  readonly count?: number | string
}

export function TabStrip<T extends string>({ items, selected, onSelect, label, idPrefix }: {
  items: readonly TabStripItem<T>[]
  selected: T
  onSelect: (id: T) => void
  label: string
  idPrefix: string
}) {
  return (
    <div className="pi-tabs" role="tablist" aria-label={label}>
      {items.map(item => {
        const active = item.id === selected
        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={`${idPrefix}-tab-${item.id}`}
            aria-selected={active}
            aria-controls={`${idPrefix}-panel`}
            tabIndex={active ? 0 : -1}
            className="pi-tabs__tab"
            onClick={() => onSelect(item.id)}
          >
            {item.label}
            {item.count !== undefined && <span className="pi-section__count" aria-hidden="true">{item.count}</span>}
          </button>
        )
      })}
    </div>
  )
}

/* --- State ----------------------------------------------------------------
 * One treatment for empty, loading and error. The three share geometry so a
 * screen does not jump between them; only the border style, the title colour
 * and the live role change.
 */
export type StateKind = 'empty' | 'loading' | 'error'

export function State({ kind, title, children, action, className }: {
  kind: StateKind
  title: string
  children?: ReactNode
  action?: ReactNode
  className?: string
}) {
  const role = kind === 'error' ? 'alert' : 'status'
  return (
    <div className={join('pi-state', kind !== 'empty' && `pi-state--${kind}`, className)} role={role}>
      <p className="pi-state__title">{title}</p>
      {children && <p className="pi-state__body">{children}</p>}
      {action && <div className="pi-state__action">{action}</div>}
    </div>
  )
}

/* --- Chips ----------------------------------------------------------------
 * A wrapping row of small pills: favorites, tags, an audience summary.
 */
export function Chips({ items, label, className }: {
  items: readonly ReactNode[]
  label?: string
  className?: string
}) {
  if (items.length === 0) return null
  return (
    <ul className={join('pi-chips', className)} {...(label ? { 'aria-label': label } : {})}>
      {items.map((item, index) => <li key={index} className="pi-chip">{item}</li>)}
    </ul>
  )
}

/* --- Steps ----------------------------------------------------------------
 * The composer's step indicator. `current` is the step id that is in progress;
 * the list keeps the ordered-list semantics of the markup it replaces.
 */
export function Steps({ items, current, label }: {
  items: readonly { readonly id: string; readonly label: string }[]
  current: string
  label: string
}) {
  return (
    <ol className="pi-steps" aria-label={label}>
      {items.map((item, index) => (
        <li key={item.id} {...(item.id === current ? { 'aria-current': 'step' as const } : {})}>
          <span aria-hidden="true">{index + 1}.</span>
          {item.label}
        </li>
      ))}
    </ol>
  )
}

/* --- Facts ----------------------------------------------------------------
 * A label/value list with symmetric dividers.
 */
export function Facts({ rows, className }: {
  rows: readonly { readonly label: string; readonly value: ReactNode }[]
  className?: string
}) {
  return (
    <dl className={join('pi-facts', className)}>
      {rows.map(row => (
        <div key={row.label} className="pi-facts__row">
          <dt className="pi-facts__label">{row.label}</dt>
          <dd className="pi-facts__value">{row.value}</dd>
        </div>
      ))}
    </dl>
  )
}

/* --- IconButton -----------------------------------------------------------
 * A square icon control with an accessible name and an optional count. The
 * icon is decorative; the label carries the meaning.
 */
export function IconButton({ icon, label, count, pressed, busy, disabled, onClick, className }: {
  icon: ReactNode
  label: string
  count?: ReactNode
  pressed?: boolean
  busy?: boolean
  disabled?: boolean
  onClick?: () => void
  className?: string
}) {
  return (
    <button
      type="button"
      className={join('pi-icon-button', className)}
      aria-label={label}
      {...(pressed === undefined ? {} : { 'aria-pressed': pressed })}
      {...(busy ? { 'aria-busy': true } : {})}
      disabled={disabled || busy}
      {...(onClick ? { onClick } : {})}
    >
      {icon}
      {count !== undefined && count !== null && <span className="pi-row__stat">{count}</span>}
    </button>
  )
}
