import { describe, expect, it } from 'vitest'
import ia from '../../../../generated/IA_EN.json'
import { adminPaths, panelRoutes, parentPath, publicPaths, resolvePanel, safeReturnTarget, tabs } from '../routes'
import { giftPath, ids } from './fixtures/ports'

const iaRoutes = ia.screens.map(screen => screen.route.replace(/\?.*$/, ''))
const participantPrefixes = ['/world', '/feed', '/meet', '/gifts', '/wall', '/me', '/notices', '/people', '/notifications', '/report']
const optionalIAPaths = new Set(['/meet/parties/:partyId/chat', '/wall/history'])
// The two routes granted by 41d90738e; the inbox is an in-panel state of the detail route.
const authorizedPanelAdditions = ['/meet/invitations/:invitationId', '/meet/invitations/:invitationId/conversation']
const requiredPanelPaths = iaRoutes.filter(route => participantPrefixes.some(prefix => route === prefix || route.startsWith(prefix + '/'))
  && !optionalIAPaths.has(route))

describe('frozen plan route composition', () => {
  it('uses the six actual top-level destinations and static siblings before identifiers', () => {
    expect(tabs.map(tab => tab.path)).toEqual(['/world', '/feed', '/meet', '/gifts', '/wall', '/me'])
    expect(resolvePanel('/gifts/mine')?.route.title).toBe('My gifts')
    expect(resolvePanel('/gifts/new')?.route.title).toBe('Create giveaway')
    expect(resolvePanel(`/gifts/pickups/${ids.gift}`)?.route.title).toBe('My pickup')
    expect(resolvePanel('/feed/new')?.route.title).toBe('New post')
  })
  it('keeps the two granted invitation routes and adds no parent list URL', () => {
    expect(authorizedPanelAdditions).toEqual(['/meet/invitations/:invitationId',
      '/meet/invitations/:invitationId/conversation'])
    expect(panelRoutes.map(route => route.path)).not.toContain('/meet/invitations')
    expect(panelRoutes.filter(route => route.path.startsWith('/meet/invitations'))).toHaveLength(2)
  })
  it('registers every required shell/public/admin IA path without implementing optional enhancements', () => {
    const paths = panelRoutes.map(route => route.path)
    expect(new Set(paths).size).toBe(paths.length)
    expect(new Set(paths)).toEqual(new Set([...requiredPanelPaths, ...authorizedPanelAdditions]))
    expect(new Set(publicPaths)).toEqual(new Set(ia.screens.filter(screen => /^A[0-9]+$/.test(screen.id) || screen.id === 'X01').map(screen => screen.route)))
    expect(new Set(adminPaths)).toEqual(new Set(iaRoutes.filter(route => route.startsWith('/admin'))))
    expect(publicPaths).toContain('/archive')
    for (const optional of optionalIAPaths) expect(paths).not.toContain(optional)
    for (const route of panelRoutes) {
      const path = route.path.replace(/:[A-Za-z]+/g, ids.gift)
      expect(resolvePanel(path)?.valid).toBe(true)
      expect(safeReturnTarget(path)).toBe(path)
    }
  })
  it('closes nested details to their parent and invalid targets to world', () => {
    expect(parentPath(`${giftPath}/manage`)).toBe(giftPath)
    expect(parentPath(giftPath)).toBe('/gifts')
    expect(parentPath('/gifts')).toBe('/world')
    expect(parentPath('/gifts/secret-handle')).toBe('/world')
  })
})

describe('return target boundary', () => {
  it.each([
    undefined, null, '', 'https://evil.example/gifts', '//evil.example/gifts', '/\\evil.example',
    '/gifts/../me', '/gifts/./new', '/gifts/%2e%2e/me', '/%252f%252fevil.example',
    '/world\n', '/world#token=secret', '/world?access_token=secret', '/gifts//new',
    '/login?returnTo=/gifts', '/admin', '/api/auth/refresh', '/me?next=https://evil.example',
    '/people/not-a-uuid', '/gifts/'+ids.gift.toUpperCase(), '/world?x=1?y=2', '/meet/missing',
  ])('rejects unsafe or unregistered input %s', input => { expect(safeReturnTarget(input)).toBe('/world') })
  it('retains an exact direct gift link and a bounded report query', () => {
    expect(safeReturnTarget(giftPath)).toBe(giftPath)
    const target = `/report?targetType=USER&targetId=${ids.user}`
    expect(safeReturnTarget(target)).toBe(target)
    expect(safeReturnTarget(target+'&targetId='+ids.other)).toBe('/world')
  })
})
