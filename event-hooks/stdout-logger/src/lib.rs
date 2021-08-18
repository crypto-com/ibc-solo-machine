use anyhow::Result;
use async_trait::async_trait;
use solo_machine_core::{
    event::{EventHandler, HandlerRegistrar},
    Event,
};

struct StdoutLogger {}

#[async_trait]
impl EventHandler for StdoutLogger {
    async fn handle(&self, event: Event) -> Result<()> {
        println!("EVENT: {:?}", event);
        Ok(())
    }
}

#[no_mangle]
pub fn register_handler(registrar: &mut dyn HandlerRegistrar) -> Result<()> {
    registrar.register(Box::new(StdoutLogger {}));
    Ok(())
}
