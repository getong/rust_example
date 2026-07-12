//! Erlang-style process layer built on kameo actors.
//!
//! - One [`player::PlayerActor`] per connected client (the "player process"),
//!   one [`map::MapActor`] per map (the "map process"). Actor mailboxes give
//!   the same data isolation as Erlang processes: all state is owned by
//!   exactly one actor and crossed only by messages.
//! - The kameo `remote` feature (libp2p swarm + distributed registry) makes
//!   the split network-transparent: `remote_message` handlers work the same
//!   whether the player actor is on this node or another one.
//! - Field-level ownership: combat buffs settle in the map tick and live in
//!   the map actor; business buffs settle in the player process and live
//!   there. Each side keeps only read-only mirrors of the other. See
//!   [`types`] for the full rules.

pub(crate) mod bridge;
pub(crate) mod map;
pub(crate) mod player;
pub(crate) mod types;
