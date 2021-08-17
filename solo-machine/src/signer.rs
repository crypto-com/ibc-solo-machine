use std::{convert::TryFrom, ffi::OsStr, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Error, Result};
use libloading::{Library, Symbol};
use solo_machine_core::{signer::SignerRegistrar as ISignerRegistrar, Signer};
use tokio::runtime::Handle;

#[derive(Default)]
pub struct SignerRegistrar {
    signer: Option<Arc<dyn Signer>>,
}

impl SignerRegistrar {
    pub fn unwrap(self) -> Result<Arc<dyn Signer>> {
        self.signer.ok_or_else(|| anyhow!("signer not registered"))
    }

    // TODO: remove conditional compilation when this issue is fixed:
    // https://github.com/nagisa/rust_libloading/issues/41
    fn register_signer(&mut self, file: impl AsRef<OsStr>) -> Result<()> {
        unsafe {
            #[cfg(target_os = "linux")]
            let library: Library = {
                // Load library with `RTLD_NOW | RTLD_NODELETE` to fix a SIGSEGV
                libloading::os::unix::Library::open(
                    Some(file),
                    libloading::os::unix::RTLD_NOW | 0x1000,
                )
                .context("unable to load signer")?
                .into()
            };
            #[cfg(not(target_os = "linux"))]
            let library = Library::new(file).context("unable to load signer")?;

            let register_fn: Symbol<
                unsafe extern "C" fn(&Handle, &mut dyn ISignerRegistrar) -> Result<()>,
            > = library
                .get("register_signer".as_bytes())
                .context("unable to load `register_signer` function from signer")?;

            let runtime = Handle::current();

            register_fn(&runtime, self)?;
        }

        Ok(())
    }
}

impl ISignerRegistrar for SignerRegistrar {
    fn register(&mut self, signer: Arc<dyn Signer>) {
        self.signer = Some(signer);
    }
}

impl TryFrom<PathBuf> for SignerRegistrar {
    type Error = Error;

    fn try_from(file: PathBuf) -> Result<Self, Self::Error> {
        let mut registrar = Self::default();
        registrar.register_signer(file)?;

        Ok(registrar)
    }
}
