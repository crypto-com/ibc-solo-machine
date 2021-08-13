pub mod cli_event_handler;

use std::{convert::TryFrom, ffi::OsStr, path::PathBuf};

use anyhow::{Context, Error, Result};
use async_trait::async_trait;
use libloading::{Library, Symbol};
use solo_machine_core::{
    event::{EventHandler, HandlerRegistrar},
    Event,
};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};

#[derive(Default)]
pub struct Registrar {
    event_handlers: Vec<Box<dyn EventHandler>>,
}

impl Registrar {
    pub fn spawn(self) -> (UnboundedSender<Event>, JoinHandle<Result<()>>) {
        let (sender, mut receiver) = unbounded_channel();

        let handle = tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                self.handle(event).await?;
            }

            Ok(())
        });

        (sender, handle)
    }

    // TODO: remove conditional compilation when this issue is fixed:
    // https://github.com/nagisa/rust_libloading/issues/41
    fn register_handler(&mut self, file: impl AsRef<OsStr>) -> Result<()> {
        unsafe {
            #[cfg(target_os = "linux")]
            let library: Library = {
                // Load library with `RTLD_NOW | RTLD_NODELETE` to fix a SIGSEGV
                libloading::os::unix::Library::open(
                    Some(file),
                    libloading::os::unix::RTLD_NOW | 0x1000,
                )
                .context("unable to load event handler")?
                .into()
            };
            #[cfg(not(target_os = "linux"))]
            let library = Library::new(file).context("unable to load event handler")?;

            let register_fn: Symbol<unsafe extern "C" fn(&mut dyn HandlerRegistrar)> = library
                .get("register_handler".as_bytes())
                .context("unable to load `register_handler` function from event hook")?;

            register_fn(self);
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for Registrar {
    async fn handle(&self, event: Event) -> Result<()> {
        // TODO: parallelise this
        for handler in self.event_handlers.iter() {
            handler.handle(event.clone()).await?;
        }

        Ok(())
    }
}

impl HandlerRegistrar for Registrar {
    fn register(&mut self, handler: Box<dyn EventHandler>) {
        self.event_handlers.push(handler)
    }
}

impl TryFrom<Vec<PathBuf>> for Registrar {
    type Error = Error;

    fn try_from(files: Vec<PathBuf>) -> Result<Self, Self::Error> {
        let mut registrar = Self::default();

        for file in files.iter() {
            registrar.register_handler(file)?;
        }

        Ok(registrar)
    }
}
