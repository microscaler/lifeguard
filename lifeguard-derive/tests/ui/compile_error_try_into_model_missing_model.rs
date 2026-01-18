//! Test that missing model attribute causes compile error
//!
//! This test verifies that when #[lifeguard(model = "...")] is missing,
//! the macro correctly reports a compile error.

use lifeguard_derive::DeriveTryIntoModel;

#[derive(DeriveTryIntoModel)]
// ERROR: Missing #[lifeguard(model = "...")] attribute
struct CreateUserRequest {
    name: String,
    email: String,
}
