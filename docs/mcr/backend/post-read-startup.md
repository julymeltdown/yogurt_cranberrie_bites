# Qualified Post read startup

Status: IN_PROGRESS. New startup targeted compilation and two configuration cases: PASS. Native/public Gateway proof: NOT_RUN.

`POST_CONCERT_READ_MODE=read` explicitly mounts the real qualified Concert read
service and its producer settlement service on the existing Post mTLS listener.
The default and `off` preserve independent inherited Post startup. The read
constructor requires at least two pool connections and the actual Auth, Friend
and Concert root callback owners. Settings are validated before database/listener
construction; production proof and configured peer services remain separate gates.

Required explicit inputs are `POST_CONCERT_READ_AUTH_GRPC_ENDPOINT`,
`POST_FRIEND_GRPC_ENDPOINT`, `POST_CONCERT_ROOT_GRPC_ENDPOINT`, actual Post mTLS,
an explicit Post service credential, Concert/Auth/Friend caller credentials,
`CONCERT_POST_READ_VERIFYING_KEYS`, and `POST_CONCERT_ROOT_READ_SIGNING_SEED`.
Endpoints require HTTPS with an explicit port and no credentials/path/query.
Only Concert can issue the qualified read methods. Only Auth/Friend can use the
qualified settlement methods. Inherited Post method policies remain independent.

The qualified source lineage is retained by merge from
`03c54fa379e5b468ea5634311714c057408e5bb5` (runtime source
`db25c4fa7db65f5f20b53ee995326b00aec029d3`), including the dedicated signed callback
capture repair. Its historical eight native component cases do not qualify
this new main-process composition. No unavailable owner, synthetic authority,
caller-supplied digest or test provider activates a successful read.

The exact clean source `c5fe8473eb08c9e40093d74c4dfba0d52eb79622` passed the bounded locked/offline Rust 1.88 check. All four ordinary cases passed across the two startup modules. Actual main executables were compiled and frozen, never executed. The build target was released before ordinary checks; all resources were released after 119.62 seconds. See [the source-bound build receipt](root-inbox-post-build.json) for commands, hashes, floors, accounting and limitations.
