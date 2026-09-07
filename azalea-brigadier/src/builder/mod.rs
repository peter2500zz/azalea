//! Typed command construction and conversion to runtime nodes.
//!
//! Built-in constructors take the parameter name directly. Each builder keeps
//! its parser type until `build`, `then`, or `register` converts it to a
//! [CommandNode](crate::tree::CommandNode). Common setters consume and return
//! the same builder type; numeric configuration therefore works after metadata,
//! requirements, handlers, or children have been attached.
//!
//! ```
//! use azalea_brigadier::prelude::*;
//! let mut dispatcher = CommandDispatcher::<()>::new();
//! dispatcher.register(
//!     literal("list").then(
//!         integer("count")
//!             .describe("How many entries")
//!             .executes(|ctx| -> CommandResult { Ok(get_integer(ctx, "count").unwrap()) })
//!             .range(1..=100),
//!     ),
//! );
//! assert_eq!(dispatcher.execute("list 5", ()).unwrap(), 5);
//! assert!(dispatcher.execute("list 101", ()).is_err());
//! ```
//!
//! # Numeric configuration
//!
//! `integer`, `long`, `float`, and `double` support `min`, `max`, and `range`.
//! All bounds are inclusive. `range` takes a non-empty `RangeInclusive` and
//! replaces both bounds; `min` and `max` replace only their respective bound.
//! Unset bounds remain unbounded. Invalid configuration (reversed or exhausted
//! ranges, contradictory bounds, or NaN) panics immediately during
//! construction. An out-of-range command input instead returns the existing
//! structured syntax error, with its cursor restored to the beginning of the
//! argument.
//!
//! # Node capabilities
//!
//! Literals and arguments share descriptions, permissions, handlers, children,
//! redirects, and forks. Only arguments support `suggests` and
//! `configure_parser`; only numeric parsers support numeric bounds. Invalid
//! method combinations are rejected by the compiler rather than by runtime
//! downcasting.
//!
//! ```compile_fail
//! use azalea_brigadier::prelude::*;
//! literal::<(), i32>("list").range(1..=10);
//! ```
//!
//! ```compile_fail
//! use azalea_brigadier::prelude::*;
//! word::<(), i32>("player").describe("Player name").min(1);
//! ```
//!
//! ```compile_fail
//! use azalea_brigadier::prelude::*;
//! integer::<(), i32>("count").range(0.5..=2.5);
//! ```
//!
//! # Migrating construction code
//!
//! - Replace `argument("count", integer())` with `integer("count")`, and
//!   `argument("enabled", bool())` with `boolean("enabled")`. The string
//!   constructors likewise take names; their tokenization rules are unchanged.
//! - Standalone parser factories now live in [crate::parsers]. Custom parsers
//!   still use `argument(name, parser)` and implement
//!   [crate::arguments::ArgumentType].
//! - An explicitly annotated argument builder includes its concrete parser as
//!   the third type parameter, for example `ArgumentBuilder<(), i32, Integer>`.
//!   Omit the annotation when inference suffices. The default kind is literal.
//! - Heterogeneous collections use built `CommandNode<S, R>` values; `register`
//!   and `then` accept either typed builders or built nodes through `Into`.
//! - `children()` replaces `arguments()`: it exposes attached children in
//!   insertion order, before same-name children are merged by `build()`.
//! - A builder is cloneable when its concrete parser is cloneable. Building a
//!   node does not require cloning its parser. Built nodes remain cloneable.
//!
//! These changes do not alter argument lookup, command return values, or the
//! distinction between synchronous handlers borrowing a context and
//! asynchronous handlers owning an Arc. No runtime or dependency is added by
//! the builders.

pub mod argument_builder;
pub mod kind;
pub mod literal_argument_builder;
pub mod required_argument_builder;
