import { useEffect, useId, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import type { PeopleController } from './controller'
import { codePointLength } from '../../shared/validation/text'
import { messages } from './runtime'
import { ReviewedProfileAvatar, type ProfileAvatarOptions } from '../profile/Avatar'
import './people.css'
export function People({ controller, avatars }: { controller: PeopleController; avatars?: ProfileAvatarOptions | undefined }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId()
  const queryLength = codePointLength(state.input.trim()), queryTooLong = queryLength > 100
  useEffect(() => { controller.start() }, [controller])
  return <section className="intro-surface intro-discovery" data-presentation="parade-ink-meet" aria-labelledby={`${id}-title`}><header><p>MEET THE COMMUNITY</p><h1 id={`${id}-title`}>PEOPLE</h1><p>Discover introductions shared with you. No social account is required.</p><nav className="intro-navigation" aria-label="People navigation"><Link to="/meet">Parties</Link><Link to="/meet/intro/edit">Your introduction</Link></nav></header>
    <form onSubmit={event => { event.preventDefault(); controller.setQuery(state.input) }}><label htmlFor={`${id}-search`}>Find introductions</label><div className="intro-actions"><input id={`${id}-search`} type="search" value={state.input} aria-describedby={`${id}-search-count`} aria-invalid={queryTooLong} onChange={event => controller.setSearch(event.target.value)} /><button type="submit" disabled={!state.available}>Search</button></div><p id={`${id}-search-count`} role={queryTooLong ? 'alert' : undefined}>{queryLength} / 100 Unicode code points after trimming.{queryTooLong && ' Shorten your search before submitting.'}</p></form>
    {!state.available ? <p role="status">People are unavailable for this session.</p> : state.page.phase === 'loading' ? <p role="status">Loading introductions…</p>
      : state.page.phase === 'error' ? <div role="status"><p>{messages[state.page.failure!]}</p><p>Visibility is checked by the server. Hidden, restricted and unavailable entries are not distinguished.</p><button onClick={() => void controller.load()}>Reload people</button><Link to="/me/age">Age &amp; privacy</Link></div>
        : state.page.data?.items.length === 0 ? <p role="status">No introductions are available for this search.</p> : null}
    <ul className="intro-people">{state.page.data?.items.map(row => <li key={row.intro_id}><ReviewedProfileAvatar config={row.author.avatar.config} label={`${row.author.display_name}'s avatar`} options={avatars} /><div><h2><Link to={`/people/${row.author.user_id}`}>{row.author.display_name}</Link></h2><p className="intro-user-text">{row.text}</p></div></li>)}</ul>
    {state.page.data?.next_cursor && <button disabled={state.page.phase === 'loading'} onClick={() => void controller.load(true)}>More people</button>}
  </section>
}
