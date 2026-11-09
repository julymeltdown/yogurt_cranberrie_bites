//! Shared guards for the profile-owned HTTP surfaces.
//!
//! Three profile surfaces — posts and media, the room, the avatar — each parse a
//! closed route set from a raw request target. Each had written its own copy of
//! the same two guards, byte for byte. A guard that differs by one spelling
//! between two surfaces is a security hole in one of them, so the guards live
//! here and every surface calls them.
//!
//! What is deliberately *not* here: the route tables themselves. Each surface's
//! table names its own operations and is more readable as a table than as a
//! shared abstraction over a table. Only the target-level predicates are shared.

/// Whether a raw target names a concert event path.
///
/// Checked on the raw target, before any route parsing, so a percent-encoded or
/// nested segment cannot slip past a later, narrower parser. `%2Fevents%2F`
/// decodes to the same path and is refused too.
///
/// Every profile-owned surface refuses an event path: a profile outlives any
/// concert, so a profile operation can never be executed under an event's
/// authority scope.
pub fn names_event_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/events/")
        || lowered.contains("%2fevents%2f")
        || lowered.contains("%2fevents/")
        || lowered.contains("/events%2f")
}

/// Whether a raw target names an intro post.
///
/// A room is never addressed through an intro post, and neither is anything else
/// that is subordinate to a profile: the intro post is a peer of the profile, not
/// a container for it. Exact segment spellings only — a word that merely starts
/// with "intro" is not an intro post, and refusing it would be a false positive.
pub fn names_intro_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/intro/")
        || lowered.contains("/intro?")
        || lowered.contains("/intros/")
        || lowered.contains("/intros?")
        || lowered.contains("/intro-posts")
        || lowered.contains("?intro=")
        || lowered.ends_with("/intro")
        || lowered.ends_with("/intros")
        || lowered.contains("%2fintro%2f")
        || lowered.contains("%2fintro/")
        || lowered.contains("/intro%2f")
        || lowered.contains("%2fintro?")
}

/// The shared target-shape refusals: length and control characters.
///
/// A target longer than 4 KiB, or one carrying a control character, is a client
/// error on every profile surface. Returns `true` when the target is acceptable.
pub fn target_is_well_formed(target: &str) -> bool {
    target.len() <= 4096 && !target.chars().any(char::is_control)
}
