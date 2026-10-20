//! Future composition root for adapters, world ownership, and task supervision.

use worldwright_core::{NotImplemented, WorldStub};

/// No successful server startup is possible in the scaffold.
pub fn start() -> Result<(), NotImplemented> {
    let mut world = WorldStub;
    world.start()
}

#[cfg(test)]
mod tests {
    #[test]
    fn startup_refuses_to_advertise_a_working_server() {
        assert!(super::start().is_err());
    }
}
