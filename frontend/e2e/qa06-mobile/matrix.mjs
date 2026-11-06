// QA06 mobile visual acceptance case matrix. Retained IA = 61 required entries
// (M10, R02, X02 are the optional P1 rows and stay out of the required count).
export const CLOCK = '2026-11-07T07:00:00.000Z'
export const WIDTHS = [360, 390, 430]
export const RELEASE = { version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) }

export const FAMILIES = [
  { id: 'PI01', name: 'Shared tokens, frame, fields, tabs, buttons, dialogs, statuses' },
  { id: 'PI02', name: 'Feed, authors, media/text-only cards, notices, reaction row, toolbar' },
  { id: 'PI03', name: 'Thread, inline replies, composers, edit/delete states' },
  { id: 'PI04', name: 'Giveaway discovery/detail/own activity, authoring, reservation, pickup' },
  { id: 'PI05', name: 'Meet discovery, applications, create/manage/rally, People, introductions' },
  { id: 'PI06', name: 'Own/public Profile, profile edit, activity and preferences' },
  { id: 'PI07', name: 'Portrait framing, avatar popover, complete avatar-editor controls' },
  { id: 'PI08', name: 'World HUD, viewport/context strip, six-tab navigation, eras' },
  { id: 'PI09', name: 'Working Wall B editor/palette/tools/confirmation, sync, degraded states' },
  { id: 'PI10', name: 'Landing, sign-in/signup/verification/reset/callback/legal/deleted' },
  { id: 'PI11', name: 'Map/place/rooms/nearby/check-in/help presentation' },
  { id: 'PI12', name: 'Social/privacy/settings links, inbox, reports, recovery, operations, archive' },
]

// id, label, family, route with fixture ids, public (no shell), state captured.
const PLACE = '00000000-0000-4000-8000-000000000030'
const POST = '00000000-0000-4000-8000-00000000003c'
const SELF_POST = '00000000-0000-4000-8000-0000000000aa'
const NOTICE = '00000000-0000-4000-8000-000000000060'
const PARTY = '00000000-0000-4000-8000-000000000010'
const GIVEAWAY = '00000000-0000-4000-8000-000000000005'
const USER = '00000000-0000-4000-8000-000000000002'
const RESERVATION = '00000000-0000-4000-8000-000000000041'
const BEACON = '00000000-0000-4000-8000-000000000070'
export const IA = [
  { id: 'A01', label: 'ENTER THE WORLD', family: 'PI10', route: '/', public: true, state: 'signed-out landing' },
  { id: 'A02', label: 'SIGN IN', family: 'PI10', route: '/login', public: true, state: 'default sign-in' },
  { id: 'A03', label: 'CREATE ACCOUNT', family: 'PI10', route: '/signup', public: true, state: 'default sign-up' },
  { id: 'A04', label: 'VERIFY EMAIL', family: 'PI10', route: '/verify-email', public: true, state: 'default verification' },
  { id: 'A05', label: 'RESET PASSWORD', family: 'PI10', route: '/reset-password', public: true, state: 'default reset' },
  { id: 'A06', label: 'CONNECTING', family: 'PI10', route: '/auth/callback', public: true, state: 'default callback' },
  { id: 'A07', label: 'TERMS / PRIVACY / SAFETY', family: 'PI10', route: '/legal/privacy', public: true, state: 'privacy document' },
  { id: 'A08', label: 'ACCOUNT DELETED', family: 'PI10', route: '/account/deleted', public: true, state: 'default deleted' },
  { id: 'W01', label: 'WORLD', family: 'PI08', route: '/world', state: 'live PARADE world' },
  { id: 'W02', label: 'MAP', family: 'PI11', route: '/world/map', state: 'default map' },
  { id: 'W03', label: 'PLACE', family: 'PI11', route: `/world/places/${PLACE}`, state: 'place detail' },
  { id: 'W04', label: 'ROOMS', family: 'PI11', route: '/world/rooms', state: 'room list' },
  { id: 'W05', label: 'NEARBY IN WORLD', family: 'PI11', route: '/world/nearby', state: 'nearby list' },
  { id: 'W06', label: 'CHOOSE AN ERA', family: 'PI08', route: '/world/theme', state: 'era selector' },
  { id: 'W07', label: 'REAL-WORLD CHECK-IN', family: 'PI11', route: '/world/check-in', state: 'check-in panel' },
  { id: 'W08', label: 'HOW TO PLAY', family: 'PI11', route: '/world/help', state: 'help panel' },
  { id: 'F01', label: 'FEED', family: 'PI02', route: '/feed', state: 'populated feed' },
  { id: 'F02', label: 'NEW POST', family: 'PI03', route: '/feed/new', state: 'composer' },
  { id: 'F03', label: 'THREAD', family: 'PI03', route: `/feed/${POST}`, state: 'thread' },
  { id: 'F04', label: 'EDIT POST', family: 'PI03', route: `/feed/${SELF_POST}/edit`, state: 'edit composer' },
  { id: 'F05', label: 'NOTICE', family: 'PI02', route: `/notices/${NOTICE}`, state: 'notice detail' },
  { id: 'M01', label: 'MEET', family: 'PI05', route: '/meet', state: 'party list' },
  { id: 'M02', label: 'CREATE PARTY', family: 'PI05', route: '/meet/parties/new', state: 'create form' },
  { id: 'M03', label: 'PARTY', family: 'PI05', route: `/meet/parties/${PARTY}`, state: 'party detail' },
  { id: 'M04', label: 'MANAGE PARTY', family: 'PI05', route: `/meet/parties/${PARTY}/manage`, state: 'owner management' },
  { id: 'M05', label: 'RENDEZVOUS', family: 'PI05', route: `/meet/parties/${PARTY}/rally`, state: 'rally panel' },
  { id: 'M06', label: 'FIND EACH OTHER', family: 'PI05', route: `/meet/beacons/${BEACON}`, state: 'beacon panel' },
  { id: 'M07', label: 'PEOPLE', family: 'PI05', route: '/meet/people', state: 'people list' },
  { id: 'M08', label: 'YOUR INTRO', family: 'PI05', route: '/meet/intro/edit', state: 'intro editor' },
  { id: 'M09', label: 'PROFILE', family: 'PI06', route: `/people/${USER}`, state: 'public profile' },
  { id: 'G01', label: 'GIFTS', family: 'PI04', route: '/gifts', state: 'browse list' },
  { id: 'G02', label: 'CREATE GIVEAWAY', family: 'PI04', route: '/gifts/new', state: 'create form' },
  { id: 'G03', label: 'GIVEAWAY', family: 'PI04', route: `/gifts/${GIVEAWAY}`, state: 'giveaway detail' },
  { id: 'G04', label: 'MANAGE GIVEAWAY', family: 'PI04', route: `/gifts/${GIVEAWAY}/manage`, state: 'owner management' },
  { id: 'G05', label: 'RESERVE', family: 'PI04', route: `/gifts/${GIVEAWAY}/reservation`, state: 'reservation form' },
  { id: 'G06', label: 'MY PICKUP', family: 'PI04', route: `/gifts/pickups/${RESERVATION}`, state: 'pickup detail' },
  { id: 'G07', label: 'MY GIFTS', family: 'PI04', route: '/gifts/mine', state: 'own activity' },
  { id: 'R01', label: 'WALL', family: 'PI09', route: '/wall', state: 'editor' },
  { id: 'U01', label: 'ME', family: 'PI06', route: '/me', state: 'own profile' },
  { id: 'U02', label: 'DRESS UP', family: 'PI07', route: '/me/avatar', state: 'avatar editor' },
  { id: 'U03', label: 'EDIT PROFILE', family: 'PI06', route: '/me/profile', state: 'profile form' },
  { id: 'U04', label: 'SOCIAL LINKS', family: 'PI12', route: '/me/social', state: 'social links' },
  { id: 'U05', label: 'REQUESTS', family: 'PI12', route: '/me/requests', state: 'requests' },
  { id: 'U06', label: 'AGE & PRIVACY', family: 'PI12', route: '/me/age', state: 'age privacy' },
  { id: 'U07', label: 'SETTINGS', family: 'PI06', route: '/me/settings', state: 'settings' },
  { id: 'U08', label: 'BLOCKED USERS', family: 'PI12', route: '/me/blocks', state: 'blocked list' },
  { id: 'U09', label: 'ACCOUNT', family: 'PI12', route: '/me/account', state: 'account controls' },
  { id: 'U10', label: 'INBOX', family: 'PI12', route: '/notifications', state: 'inbox list' },
  { id: 'X01', label: 'AFTER THE SHOW', family: 'PI12', route: '/archive', state: 'archive' },
  { id: 'S01', label: 'REPORT', family: 'PI12', route: `/report?targetType=POST&targetId=${POST}`, state: 'report form' },
  { id: 'S02', label: 'UNAVAILABLE', family: 'PI12', route: `/gifts/${GIVEAWAY}`, state: 'route error boundary', mode: 'error' },
  { id: 'S03', label: 'RECONNECTING / MAINTENANCE', family: 'PI12', route: '/world', state: 'connection overlay', mode: 'offline' },
  { id: 'O01', label: 'CONTROL ROOM', family: 'PI12', route: '/admin', state: 'operations' },
  { id: 'O02', label: 'EVENT SETTINGS', family: 'PI12', route: '/admin/event', state: 'operations' },
  { id: 'O03', label: 'VENUE & PLACES', family: 'PI12', route: '/admin/places', state: 'operations' },
  { id: 'O04', label: 'REPORT QUEUE', family: 'PI12', route: '/admin/reports', state: 'operations' },
  { id: 'O05', label: 'MEMBER REVIEW', family: 'PI12', route: `/admin/users/${USER}`, state: 'operations' },
  { id: 'O06', label: 'CONTENT REVIEW', family: 'PI12', route: '/admin/content', state: 'operations' },
  { id: 'O07', label: 'WALL CONTROL', family: 'PI12', route: '/admin/mural', state: 'operations' },
  { id: 'O08', label: 'AUDIT LOG', family: 'PI12', route: '/admin/audit', state: 'operations' },
  { id: 'O09', label: 'CLOSE THE EVENT', family: 'PI12', route: '/admin/archive', state: 'operations' },
]

// Additional family states not expressed by one IA route alone.
export const FAMILY_CASES = [
  { id: 'PI08-revenge-era', family: 'PI08', route: '/world', era: 'REVENGE', state: 'REVENGE era HUD' },
  { id: 'PI08-danger-era', family: 'PI08', route: '/world', era: 'DANGER', state: 'DANGER era HUD' },
  { id: 'PI02-text-only', family: 'PI02', route: '/feed', state: 'feed text-only card' },
  { id: 'PI03-inline-reply', family: 'PI03', route: `/feed/${POST}`, state: 'thread reply composer' },
  { id: 'PI01-status', family: 'PI01', route: '/admin', state: 'shared unavailable/status chrome' },
  { id: 'PI09-grid', family: 'PI09', route: '/wall', state: 'wall grid and palette' },
]
