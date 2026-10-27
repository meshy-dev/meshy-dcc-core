use std::path::PathBuf;

use crate::cli::UpdateAction;

const SERVER_BINARY_NAME: &str = env!("CARGO_BIN_NAME");

pub(crate) async fn run_update_cmd(gateway_port: u16, action: UpdateAction) -> anyhow::Result<()> {
    let gateway_url = format!("http://127.0.0.1:{gateway_port}");

    match action {
        UpdateAction::Check {
            binary,
            current_version,
        } => {
            let binary_name = binary.unwrap_or_else(|| SERVER_BINARY_NAME.to_string());
            let current_version = current_version
                .unwrap_or_else(|| meshy_dcc_updater::meshy_release::PRODUCT_VERSION.to_string());
            let updater =
                meshy_dcc_updater::Updater::new(&gateway_url, &binary_name, &current_version);
            let info = updater.check_update().await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        UpdateAction::Apply => stage_server_update(&gateway_url).await?,
    }
    Ok(())
}

async fn stage_server_update(gateway_url: &str) -> anyhow::Result<()> {
    let updater =
        meshy_dcc_updater::Updater::new(gateway_url, SERVER_BINARY_NAME, env!("CARGO_PKG_VERSION"));
    let info = updater.check_update().await?;
    if !info.update_available {
        print_up_to_date(&info)?;
        return Ok(());
    }
    let downloaded = updater.download_verified_update(&info).await?;
    meshy_dcc_updater::Updater::stage_verified_update(
        downloaded.path(),
        updater.binary_name(),
        downloaded.sha256(),
    )?;
    print_staged(&info, &[downloaded.path().to_path_buf()])?;
    Ok(())
}

fn print_up_to_date(info: &meshy_dcc_updater::UpdateInfo) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "up-to-date",
            "current_version": info.current_version,
            "latest_version": info.latest_version,
            "message": "Already running the latest version."
        }))?
    );
    Ok(())
}

fn print_staged(
    info: &meshy_dcc_updater::UpdateInfo,
    downloaded: &[PathBuf],
) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "staged",
            "current_version": info.current_version,
            "latest_version": info.latest_version,
            "staged_at": downloaded.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>(),
            "message": "Update downloaded and staged. Restart the server to apply it.",
        }))?
    );
    Ok(())
}

/// Apply a staged update and report whether the process must re-exec.
pub(crate) fn apply_staged_update() -> anyhow::Result<bool> {
    match meshy_dcc_updater::Updater::apply_staged_update(SERVER_BINARY_NAME) {
        Ok(true) => {
            tracing::info!("staged server update applied; restarting into the new executable");
            Ok(true)
        }
        Ok(false) => Ok(false),
        Err(
            meshy_dcc_updater::UpdateError::LegacyUnsignedStage
            | meshy_dcc_updater::UpdateError::RejectedStagedUpdate { .. },
        ) => {
            tracing::warn!("refused and quarantined an untrusted staged server update");
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

/// Run the just-installed executable in place of this process, with the
/// exact current arguments. Returns only when it could not be started.
pub(crate) fn restart_after_update() -> anyhow::Result<()> {
    let error = meshy_dcc_updater::relaunch_current_exe(&[]);
    anyhow::bail!("could not start the updated server executable: {error}")
}
