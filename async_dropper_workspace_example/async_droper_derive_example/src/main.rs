use std::{result::Result, time::Duration};

use async_dropper::derive::AsyncDrop;
use async_trait::async_trait;

/// This object will be async-dropped
///
/// Objects that are dropped *must* implement [Default] and [PartialEq]
/// (so make members optional, hide them behind Rc/Arc as necessary)
#[derive(Debug, Default, PartialEq, Eq, AsyncDrop)]
struct AsyncThing(String);

/// Implementation of [AsyncDrop] that specifies the actual behavior
#[async_trait]
impl AsyncDrop for AsyncThing {
    // simulated work during async_drop
    async fn async_drop(&mut self) -> Result<(), AsyncDropError> {
        eprintln!("async dropping [{}]!", self.0);
        tokio::time::sleep(Duration::from_secs(2)).await;
        eprintln!("dropped [{}]!", self.0);
        Ok(())
    }

    // This function serves to indicate when async_drop behavior should *not* be performed
    // (i.e., if Self::default == Self, a drop must have occurred, or does not need to)
    fn reset(&mut self) {
        self.0 = String::default();
    }

    // How long we can allow async drop behavior to block
    fn drop_timeout(&self) -> Duration {
        Duration::from_secs(5) // extended from default 3 seconds
    }

    // NOTE: below was not implemented since we want the default of DropFailAction::Continue
    // fn drop_fail_action(&self) -> DropFailAction;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        let _example_obj = AsyncThing(String::from("test"));
        eprintln!("here comes the (async) drop");
        // drop will be triggered here
        // you could also call `drop(_example_obj)`
    }

    Ok(())
}
