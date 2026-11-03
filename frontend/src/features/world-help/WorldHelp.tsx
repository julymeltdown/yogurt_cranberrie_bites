import { ShellLink } from '../../app/navigation'
import './world-help.css'

export function WorldHelp() {
  return <article className="world-help" aria-label="Controls and meeting safety">
    <header><p className="world-help-kicker">How to play</p><h2>One world, at your pace</h2>
      <p>Explore on screen. Make real meeting decisions separately.</p></header>
    <section><h3>Move around</h3><dl>
      <div><dt>Touch</dt><dd>Drag the movement joystick and release to stop. Tap a walkable spot on the map to walk toward it. Dragging the map is not a walk command.</dd></div>
      <div><dt>Keyboard</dt><dd>Focus the map or movement joystick. Use arrow keys or W, A, S, D to move. Release the keys to stop; Escape cancels movement.</dd></div>
      <div><dt>Explore a place</dt><dd>Choose a nearby interaction in the labeled list, then use Interact, or press E while the map or joystick is focused. Map → place details also works without precise canvas input.</dd></div>
    </dl><p>Movement pauses while a panel or text field is active, when the world is hidden, or when focus leaves the controls. It does not resume until you give fresh input.</p></section>
    <section><h3>Look around</h3><p>Zoom in and Zoom out change the local view. Recenter follows your avatar again; R also recenters while a movement control is focused. Show on map changes the camera only, without moving your avatar.</p>
      <p>A release mismatch pauses map targets. Return to the world and reload the current map and places; do not follow an old marker.</p></section>
    <section><h3>Virtual is not physical</h3><p>A virtual concept is a game landmark, never a verified physical meeting destination. Physical references show their verification, source review, and closure state.</p>
      <p>Walking an avatar does not confirm real arrival, admission, GPS position, distance or a safe route. Closed, unverified or source-unavailable places should not be treated as verified meeting guidance.</p></section>
    <section><h3>Meet with care</h3><p>Use current venue guidance and an open, reviewed place. Agree on the audience and time explicitly. Physical meeting guidance needs a current revision, last confirmation and expiry; old or moved guidance may no longer apply.</p>
      <p>Share only what you intend with the chosen audience. A photo permission or a source review is not a guarantee of current access or safety. Follow staff instructions, and leave a meeting if you feel uncomfortable.</p></section>
    <nav aria-label="Explore after reading"><ShellLink id="help-map" to="/world/map">Browse places</ShellLink>
      <ShellLink id="help-world" to="/world">Return to world</ShellLink></nav>
  </article>
}
