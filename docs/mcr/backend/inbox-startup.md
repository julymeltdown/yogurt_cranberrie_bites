# Shared inbox startup

Status: IN_PROGRESS. Compilation and native/public HTTP qualification: NOT_RUN.

`CONCERT_NOTIFICATION_INBOX_MODE=live` explicitly adds the four frozen inbox
operations to the existing shared live Intro/Participant production composition.
The default and `off` preserve independent startup. Invalid values or activation
without shared Intro fail before database/listener construction.

The configured `NotificationClient`, canonical fence, ledger, registry and
delivery publisher all come from the same `CanonicalOwnerComposition`. Each
inbox request uses `ScopedProductOwner`; other operations retain the existing
Intro admission. No extra owner acquisition occurs inside a producer, and no
second delivery service, completion capability or fallback inbox is installed.

This source increment does not establish Notification target authority,
authenticated notification fanout, or the public Gateway journey. The original
owners are implementing those dependencies separately. Missing owners must
remain unavailable, including empty inboxes and unread counts. The real
Notification child remains retained through the existing Root delivery and
finished-root recovery path.

Validation is pending a fresh bounded resource offer for the combined source.
The earlier qualified Intro-only startup is not evidence for this new opt-in.
