import type { HttpClient, HttpRequest } from '../../../shared/api/http'
import { decodeLikeState, type IntroLikeState } from '../contract'

/**
 * WP-05 T-411: the like API.
 *
 * `PUT` sets a heart, `DELETE` clears it. Both are idempotent on the server, so
 * a retried request is safe; the *client* still guards against sending two in
 * flight at once, because two requests are two round trips and the user sees the
 * second one's latency.
 */
export interface LikeTransport {
  readonly request: HttpClient['request']
}

export function setLike(transport: LikeTransport, introId: string, signal?: AbortSignal): Promise<IntroLikeState> {
  const request: HttpRequest<IntroLikeState> = {
    path: `/api/intro-posts/${introId}/like`,
    method: 'PUT',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'setIntroPostLike',
    decode: decodeLikeState,
    ...(signal === undefined ? {} : { signal }),
  }
  return transport.request(request).then((result) => result.data)
}

export function clearLike(transport: LikeTransport, introId: string, signal?: AbortSignal): Promise<IntroLikeState> {
  const request: HttpRequest<IntroLikeState> = {
    path: `/api/intro-posts/${introId}/like`,
    method: 'DELETE',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'clearIntroPostLike',
    decode: decodeLikeState,
    ...(signal === undefined ? {} : { signal }),
  }
  return transport.request(request).then((result) => result.data)
}
