//! Reject updates requiring unseen server-side elevation before draining requests.
use super::PayloadKind;
use anyhow::Result;
use std::path::Path;

pub(super) fn check(
    payload: &PayloadKind,
    target: &Path,
    requires_elevation: impl FnOnce(&Path) -> Result<bool>,
) -> Result<()> {
    if matches!(
        payload,
        PayloadKind::Installer {
            kind: super::InstallerKind::WindowsSetup,
            ..
        }
    ) && requires_elevation(target)?
    {
        anyhow::bail!("This installation requires administrator privileges. Please manually run the installer on the server, or restart llmlb as administrator before updating. The server has not been stopped.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::InstallerKind;

    fn installer() -> PayloadKind {
        PayloadKind::Installer {
            installer_path: "setup.exe".into(),
            kind: InstallerKind::WindowsSetup,
        }
    }

    #[test]
    fn elevation_required_is_rejected_with_recovery_guidance() {
        let error = check(&installer(), Path::new("llmlb.exe"), |_| Ok(true)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("administrator"));
        assert!(message.contains("manually"));
        assert!(message.contains("server"));
    }

    #[test]
    fn per_user_and_already_elevated_installer_are_allowed() {
        check(&installer(), Path::new("llmlb.exe"), |_| Ok(false)).unwrap();
    }

    #[test]
    fn portable_update_does_not_request_elevation() {
        check(
            &PayloadKind::Portable {
                binary_path: "new.exe".into(),
            },
            Path::new("llmlb.exe"),
            |_| panic!("portable must not request elevation"),
        )
        .unwrap();
    }

    #[test]
    fn indeterminate_permissions_fail_closed() {
        assert!(
            check(&installer(), Path::new("llmlb.exe"), |_| anyhow::bail!(
                "unknown installation mode"
            ))
            .is_err()
        );
    }
}
