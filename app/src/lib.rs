//! JSON command API. `dispatch(session, "person.get", args)` is the only entry point the UI uses.

mod api;
pub use api::{dispatch, ApiError, Session};
