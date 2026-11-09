import { cleanQuery, send, type RequestPort } from '../../../shared/api/request'
import {
  decodeCommentPage,
  decodeIntroComment,
  decodeIntroPostDetail,
  decodeIntroPostPage,
  decodeLikerPage,
  decodeOwnIntroPost,
  type IntroCommentPage,
  type IntroComment,
  type IntroPostDetail,
  type IntroPostPage,
  type LikerPage,
  type OwnIntroPost,
  type SaveIntroPost,
} from '../contract'

/**
 * WP-05 T-405 client: the 13 intro operations.
 *
 * Every request names its frozen `domainOperation`, so the shared transport
 * validates the method, path and query against the generated registry before
 * anything is sent. A typo in a path fails locally rather than reaching the
 * server.
 */
export type IntroTransport = RequestPort

export function getIntroPost(transport: IntroTransport, introId: string): Promise<IntroPostDetail> {
  return send(transport, {
    path: `/api/intro-posts/${introId}`,
    method: 'GET',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'getIntroPost',
    decode: decodeIntroPostDetail,
  })
}

export function listIntroPosts(
  transport: IntroTransport,
  query: { limit?: number; cursor?: string; q?: string },
): Promise<IntroPostPage> {
  return send(transport, {
    path: '/api/discover/intro-posts',
    method: 'GET',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'listIntroPosts',
    query: cleanQuery(query),
    decode: decodeIntroPostPage,
  })
}

export function getOwnIntroPost(transport: IntroTransport, introId: string): Promise<OwnIntroPost> {
  return send(transport, {
    path: `/api/me/intro-posts/${introId}`,
    method: 'GET',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'getOwnIntroPost',
    decode: decodeOwnIntroPost,
  })
}

export function saveIntroPostDraft(
  transport: IntroTransport,
  introId: string,
  body: SaveIntroPost,
  idempotencyKey: string,
): Promise<OwnIntroPost> {
  return send(transport, {
    path: `/api/me/intro-posts/${introId}`,
    method: 'PUT',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'saveIntroPostDraft',
    idempotencyKey,
    body: { json: body as never },
    decode: decodeOwnIntroPost,
  })
}

export function publishIntroPost(
  transport: IntroTransport,
  introId: string,
  expectedRevision: number,
  idempotencyKey: string,
): Promise<OwnIntroPost> {
  return send(transport, {
    path: `/api/me/intro-posts/${introId}/publish`,
    method: 'POST',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'publishIntroPost',
    idempotencyKey,
    body: { json: { expected_revision: expectedRevision } as never },
    decode: decodeOwnIntroPost,
  })
}

export function deleteIntroPost(
  transport: IntroTransport,
  introId: string,
  expectedRevision: number,
  idempotencyKey: string,
): Promise<unknown> {
  return send(transport, {
    path: `/api/me/intro-posts/${introId}`,
    method: 'DELETE',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'deleteIntroPost',
    idempotencyKey,
    query: { expected_revision: expectedRevision },
    decode: (value: unknown) => value,
  })
}

export function listIntroPostLikers(
  transport: IntroTransport,
  introId: string,
  query: { limit?: number; cursor?: string },
): Promise<LikerPage> {
  return send(transport, {
    path: `/api/intro-posts/${introId}/likes`,
    method: 'GET',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'listIntroPostLikers',
    query: cleanQuery(query),
    decode: decodeLikerPage,
  })
}

export function listIntroPostComments(
  transport: IntroTransport,
  introId: string,
  query: { limit?: number; cursor?: string },
): Promise<IntroCommentPage> {
  return send(transport, {
    path: `/api/intro-posts/${introId}/comments`,
    method: 'GET',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'listIntroPostComments',
    query: cleanQuery(query),
    decode: decodeCommentPage,
  })
}

export function createIntroPostComment(
  transport: IntroTransport,
  introId: string,
  body: string,
  parentCommentId: string | null,
  idempotencyKey: string,
): Promise<IntroComment> {
  return send(transport, {
    path: `/api/intro-posts/${introId}/comments`,
    method: 'POST',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'createIntroPostComment',
    idempotencyKey,
    body: { json: { body, parent_comment_id: parentCommentId } as never },
    decode: decodeIntroComment,
  })
}

export function deleteIntroPostComment(
  transport: IntroTransport,
  introId: string,
  commentId: string,
  idempotencyKey: string,
): Promise<unknown> {
  return send(transport, {
    path: `/api/intro-posts/${introId}/comments/${commentId}`,
    method: 'DELETE',
    access: 'protected',
    errorFamily: 'domain',
    domainOperation: 'deleteIntroPostComment',
    idempotencyKey,
    decode: (value: unknown) => value,
  })
}

