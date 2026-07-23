# ADR-0003: Sockets (and `/dev/tcp`-style redirection) stay out of scope until a consumer need exists

Status: Accepted
Date: 2026-07-23

## Context

REVIEW.md Round 4 raised a design question rather than a scored gap (see
[issue #72](https://github.com/baileyrd/rusty_libc/issues/72), same spirit as
#33's `signalfd` question): bash and several bash-compatible shells support
`exec 3<>/dev/tcp/host/port`-style redirection. Supporting that in `rush`
would need a real sockets API from this crate — at minimum `socket`,
`connect`, and some form of address resolution (`getaddrinfo`-shaped DNS
lookup) — plus its own family of design questions this crate hasn't had to
answer for anything else so far: blocking vs. non-blocking `connect`,
IPv4/IPv6 handling, and mapping resolver failures onto `Errno`, whose whole
design assumes a kernel `-errno` return, not `getaddrinfo`'s separate
`EAI_*` error space.

This is a materially different shape of decision than ADR-0002's. `signalfd`
was "the crate already fully supports the underlying capability
(`sigaction`/`sigprocmask`); should a second, better-suited API become the
recommended path for it?" — the consumer need (react to `SIGCHLD`/`SIGINT`/
`SIGWINCH`/etc.) was never in question, only which primitive should be
recommended for it. Sockets are the opposite: this crate has *zero*
networking surface today, and Round 4's review found no evidence `rush`
implements `/dev/tcp` redirection, calls it as a builtin, or has it on a
near-term roadmap. There is no confirmed consumer need to build a primitive
for — only a plausible future one.

## Decision

**Sockets stay out of scope.** This crate will not add `socket`/`connect`/
address-resolution primitives speculatively. The trigger to revisit is
concrete: `rush` (or another consumer) actually wanting `/dev/tcp`/`/dev/udp`
redirection, a socket-based IPC mechanism, or some other networking-shaped
feature. At that point this ADR should be superseded by one that scopes the
real primitive against the real caller, rather than guessing at blocking
semantics, address-family coverage, and error mapping in the abstract now.

This is the same reasoning already applied, without a dedicated ADR, to
Round 4's "Considered and not proposed" list (advisory locking, `chroot`/
namespaces/seccomp, `sendfile`/`splice`, extended attributes) — no identified
consumer, so no speculative surface. Sockets got their own issue and ADR
rather than a one-line dismissal because the surface is large enough, and
the design questions specific enough (blocking-vs-async `connect`, address
families, `EAI_*`-to-`Errno` mapping), that the *reasoning* for deferring is
worth recording, not just the outcome.

## Alternatives considered

- **Add a minimal `socket`/`connect`/blocking-DNS primitive now, scoped
  narrowly to what `/dev/tcp` needs.** Rejected: "narrowly scoped" is doing a
  lot of work in that sentence with no real caller to scope against. Every
  open question (does `connect` need a timeout? does `rush` want IPv6? what
  should a resolution failure look like as an `Errno`?) would be answered by
  guessing at what a future `/dev/tcp` builtin wants rather than what it
  actually needs — precisely the speculative-surface pattern this crate has
  consistently avoided elsewhere (see the Round 4 "Considered and not
  proposed" list).
- **Declare networking permanently out of scope for this crate.** Rejected:
  too strong. Nothing about the crate's raw-syscall, kernel-layout, per-arch
  design is networking-hostile, and a job-control shell wanting `/dev/tcp`
  redirection is a reasonable, precedented feature (bash has shipped it for
  decades). "Permanently out of scope" would just mean re-deciding this same
  question again later under time pressure once a consumer actually wants
  it, which is a worse position than deferring with the reasoning already
  written down.
- **Leave it unstated, closing the issue with no ADR.** Rejected for the same
  reason ADR-0002 exists at all: the point of writing these down is that the
  next person (including a future instance of whoever is triaging this
  crate) shouldn't have to re-derive "why doesn't rusty_libc have sockets"
  from scratch, or worse, not realize it was already considered and re-open
  the same debate.

## Consequences

- No code changes from this ADR. No `socket`/`connect`/resolver module is
  added.
- REVIEW.md Round 4's design-question section is updated to point here,
  matching how Round 3 §L points to ADR-0002.
- The concrete, checkable trigger for revisiting: a `rush` (or other
  consumer) issue or PR that actually wants `/dev/tcp`/`/dev/udp` redirection
  or another networking-shaped feature. Until then, this crate's networking
  surface remains zero by design, not by omission.
