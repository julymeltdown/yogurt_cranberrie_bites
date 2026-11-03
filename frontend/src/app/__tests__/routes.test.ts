import { describe, expect, it } from 'vitest'
import { adminPaths, panelRoutes, parentPath, publicPaths, resolvePanel, safeReturnTarget, tabs } from '../routes'
import { giftPath, ids } from './fixtures/ports'

describe('frozen plan route composition', () => {
  it('uses the six actual top-level destinations and static siblings before identifiers', () => {
    expect(tabs.map(tab => tab.path)).toEqual(['/world', '/feed', '/meet', '/gifts', '/wall', '/me'])
    expect(resolvePanel('/gifts/mine')?.route.title).toBe('My gifts')
    expect(resolvePanel('/gifts/new')?.route.title).toBe('Create giveaway')
    expect(resolvePanel(`/gifts/pickups/${ids.gift}`)?.route.title).toBe('My pickup')
    expect(resolvePanel('/feed/new')?.route.title).toBe('New post')
  })
  it('registers every required shell/public/admin IA path without implementing optional enhancements', () => {
    expect(panelRoutes).toHaveLength(41)
    expect(publicPaths).toContain('/auth/callback')
    expect(publicPaths).toContain('/archive')
    expect(adminPaths).toHaveLength(9)
    expect(panelRoutes.some(route => route.path === '/wall/history')).toBe(false)
    expect(panelRoutes.some(route => route.path.endsWith('/chat'))).toBe(false)
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
