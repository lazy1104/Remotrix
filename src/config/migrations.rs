use std::path::PathBuf;

use super::paths::{copy_dir, copy_dir_contents, copy_file, resolve_from_settings};
use super::settings::Settings;

const LEGACY_BUNDLED_FONT: &str = "HarmonyOS Sans SC";

/// Reconcile `settings.font_family` on boot:
///
/// - Legacy `"HarmonyOS Sans SC"` (carried over from builds that bundled
///   the CJK font) is rewritten to empty so the OS-following branch
///   below picks it up.
/// - Empty means "follow the OS" and is left alone; the boot path
///   resolves it transiently via `font_autopick::pick_default_family`
///   without persisting.
/// - Any other value is a user-explicit family choice and is preserved.
pub fn migrate_font_family(settings: &mut Settings) {
    if settings.font_family == LEGACY_BUNDLED_FONT {
        tracing::info!("legacy bundled font cleared to follow OS at boot");
        settings.font_family = String::new();
        return;
    }
    if settings.font_family.is_empty() {
        tracing::info!("font_family empty; will resolve OS sans at boot (not persisted)");
    }
}

/// Rewrite the long-deprecated gruk.org default bootstrap URLs to the
/// current emule-security mirrors. Only matches the exact old default
/// strings; user-customised URLs are left untouched. Returns `true` when
/// any field was rewritten so the caller knows to persist.
pub(crate) fn fix_dead_ed2k_bootstrap_urls(settings: &mut Settings) -> bool {
    const OLD_SERVER_MET: &str = "http://www.gruk.org/server.met";
    const OLD_NODES_DAT: &str = "http://www.gruk.org/nodes.dat";
    const NEW_SERVER_MET: &str = "https://upd.emule-security.org/server.met";
    const NEW_NODES_DAT: &str = "https://upd.emule-security.org/nodes.dat";
    let mut changed = false;
    if settings.aria2.ed2k_server_met_url == OLD_SERVER_MET {
        settings.aria2.ed2k_server_met_url = NEW_SERVER_MET.to_string();
        changed = true;
    }
    if settings.aria2.ed2k_nodes_dat_url == OLD_NODES_DAT {
        settings.aria2.ed2k_nodes_dat_url = NEW_NODES_DAT.to_string();
        changed = true;
    }
    changed
}

/// Migrate data from previous on-disk paths into the newly configured
/// ones, when the user has changed a path override. Called once on
/// startup before [`crate::logging::init`], so the log writer constructs
/// against the already-migrated location.
///
/// Behaviour:
/// - `aria2_bin_dir` and `log_dir`: full recursive copy of every file
///   under the old directory. Skips any individual file that already
///   exists at the destination (so a non-empty target is partially
///   honoured rather than overwritten).
/// - `app_data_dir`: only a fixed whitelist of files/dirs (`remotrix.db`,
///   `remotrix.db-journal`, `ed2k-bootstrap`, `ed2k-search`) is moved.
///   The internal `aria2/` and `logs/` directories are intentionally
///   skipped — they have their own override switches and are migrated
///   by the two preceding rules.
///
/// Errors are returned as `String` so the caller can surface them
/// without dragging `anyhow` into the boot path; missing source paths
/// and equal old/new paths are silent no-ops.
pub fn migrate_paths(settings: &mut Settings) -> Result<(), String> {
    let new = resolve_from_settings(settings);

    for (label, old, new_path) in [
        (
            "aria2_bin_dir",
            &settings.last_resolved.aria2_bin_dir,
            &new.aria2_bin_dir,
        ),
        ("log_dir", &settings.last_resolved.log_dir, &new.log_dir),
    ] {
        if old == new_path {
            continue;
        }
        if !old.exists() {
            continue;
        }
        copy_dir_contents(old, new_path).map_err(|e| {
            format!(
                "path migration {label} {} -> {}: {e}",
                old.display(),
                new_path.display()
            )
        })?;
        tracing::info!(
            label,
            old = %old.display(),
            new = %new_path.display(),
            "path migration complete"
        );
    }

    if settings.last_resolved.app_data_dir != new.app_data_dir {
        let old = &settings.last_resolved.app_data_dir;
        if old.exists() {
            for entry in [
                "remotrix.db",
                "remotrix.db-journal",
                "ed2k-bootstrap",
                "ed2k-search",
            ] {
                let src = old.join(entry);
                if !src.exists() {
                    continue;
                }
                let dst = new.app_data_dir.join(entry);
                if dst.exists() {
                    continue;
                }
                if src.is_dir() {
                    copy_dir(&src, &dst).map_err(|e| {
                        format!(
                            "path migration app_data {}/{} -> {}/{}: {e}",
                            old.display(),
                            entry,
                            new.app_data_dir.display(),
                            entry
                        )
                    })?;
                } else {
                    copy_file(&src, &dst).map_err(|e| {
                        format!(
                            "path migration app_data {}/{} -> {}/{}: {e}",
                            old.display(),
                            entry,
                            new.app_data_dir.display(),
                            entry
                        )
                    })?;
                }
            }
            tracing::info!(
                old = %old.display(),
                new = %new.app_data_dir.display(),
                "path migration complete"
            );
        }
    }

    settings.last_resolved = new;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::ResolvedPaths;
    use crate::config::Settings;

    fn unique_tmp(label: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("remotrix-cfg-{label}-{pid}-{nanos}"))
    }

    #[test]
    fn migrate_paths_copies_aria2_dir() {
        let old = unique_tmp("aria2-old");
        let new = unique_tmp("aria2-new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(".installed"), "{}").unwrap();
        std::fs::write(old.join("session.txt"), "abc").unwrap();
        std::fs::write(old.join("aria2-next-1.0-linux"), "bin").unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: old.clone(),
                app_data_dir: old.clone(),
                log_dir: old.clone(),
            },
            ..Settings::default()
        };
        settings.paths.aria2_bin_dir = Some(new.clone());

        migrate_paths(&mut settings).unwrap();

        assert!(new.join(".installed").exists());
        assert_eq!(
            std::fs::read_to_string(new.join("session.txt")).unwrap(),
            "abc"
        );
        assert_eq!(
            std::fs::read_to_string(new.join("aria2-next-1.0-linux")).unwrap(),
            "bin"
        );
        assert_eq!(settings.last_resolved.aria2_bin_dir, new);

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migrate_paths_copies_log_files() {
        let old = unique_tmp("log-old");
        let new = unique_tmp("log-new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("remotrix.2026-08-25.log"), "old").unwrap();
        std::fs::write(old.join("aria2.2026-08-25.log"), "old").unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: old.clone(),
                app_data_dir: old.clone(),
                log_dir: old.clone(),
            },
            ..Settings::default()
        };
        settings.paths.log_dir = Some(new.clone());

        migrate_paths(&mut settings).unwrap();

        assert_eq!(
            std::fs::read_to_string(new.join("remotrix.2026-08-25.log")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(new.join("aria2.2026-08-25.log")).unwrap(),
            "old"
        );

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migrate_paths_app_data_whitelist() {
        let old = unique_tmp("appdata-old");
        let new = unique_tmp("appdata-new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(old.join("aria2")).unwrap();
        std::fs::create_dir_all(old.join("logs")).unwrap();
        std::fs::write(old.join("remotrix.db"), "db").unwrap();
        std::fs::write(old.join("aria2").join("keep.bin"), "bin").unwrap();
        std::fs::write(old.join("logs").join("keep.log"), "log").unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: old.clone(),
                app_data_dir: old.clone(),
                log_dir: old.clone(),
            },
            ..Settings::default()
        };
        settings.paths.app_data_dir = Some(new.clone());

        migrate_paths(&mut settings).unwrap();

        assert!(new.join("remotrix.db").exists());
        assert!(
            !new.join("aria2").exists(),
            "aria2/ must not be migrated here"
        );
        assert!(
            !new.join("logs").exists(),
            "logs/ must not be migrated here"
        );

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migrate_paths_skips_existing_files_at_target() {
        let old = unique_tmp("aria2-old2");
        let new = unique_tmp("aria2-new2");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(".installed"), "{}").unwrap();
        std::fs::write(old.join("aria2-next-2.0-linux"), "newer").unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("aria2-next-2.0-linux"), "existing").unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: old.clone(),
                app_data_dir: old.clone(),
                log_dir: old.clone(),
            },
            ..Settings::default()
        };
        settings.paths.aria2_bin_dir = Some(new.clone());

        migrate_paths(&mut settings).unwrap();

        assert_eq!(
            std::fs::read_to_string(new.join("aria2-next-2.0-linux")).unwrap(),
            "existing",
            "existing target file must not be overwritten"
        );
        assert!(new.join(".installed").exists());

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migrate_paths_noop_when_unchanged() {
        let same = unique_tmp("noop");
        std::fs::create_dir_all(&same).unwrap();
        std::fs::write(same.join("marker"), "x").unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: same.clone(),
                app_data_dir: same.clone(),
                log_dir: same.clone(),
            },
            ..Settings::default()
        };

        migrate_paths(&mut settings).unwrap();

        assert_eq!(
            std::fs::read_to_string(same.join("marker")).unwrap(),
            "x",
            "nothing should have been copied"
        );
        let _ = std::fs::remove_dir_all(&same);
    }

    #[test]
    fn migrate_paths_handles_missing_old() {
        let old = unique_tmp("missing-old");
        let new = unique_tmp("missing-new");
        std::fs::create_dir_all(&new).unwrap();

        let mut settings = Settings {
            last_resolved: ResolvedPaths {
                aria2_bin_dir: old.clone(),
                app_data_dir: old.clone(),
                log_dir: old.clone(),
            },
            ..Settings::default()
        };
        settings.paths.aria2_bin_dir = Some(new.clone());
        settings.paths.log_dir = Some(new.clone());
        settings.paths.app_data_dir = Some(new.clone());

        migrate_paths(&mut settings).unwrap();

        assert_eq!(settings.last_resolved.aria2_bin_dir, new);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn fix_dead_ed2k_bootstrap_urls_rewrites_gruk_defaults() {
        let mut settings = Settings::default();
        settings.aria2.ed2k_server_met_url = "http://www.gruk.org/server.met".into();
        settings.aria2.ed2k_nodes_dat_url = "http://www.gruk.org/nodes.dat".into();
        assert!(fix_dead_ed2k_bootstrap_urls(&mut settings));
        assert_eq!(
            settings.aria2.ed2k_server_met_url,
            "https://upd.emule-security.org/server.met"
        );
        assert_eq!(
            settings.aria2.ed2k_nodes_dat_url,
            "https://upd.emule-security.org/nodes.dat"
        );
    }

    #[test]
    fn fix_dead_ed2k_bootstrap_urls_preserves_user_urls() {
        let mut settings = Settings::default();
        settings.aria2.ed2k_server_met_url = "https://my-mirror.example/server.met".into();
        settings.aria2.ed2k_nodes_dat_url = "https://upd.emule-security.org/nodes.dat".into();
        assert!(!fix_dead_ed2k_bootstrap_urls(&mut settings));
        assert_eq!(
            settings.aria2.ed2k_server_met_url,
            "https://my-mirror.example/server.met"
        );
        assert_eq!(
            settings.aria2.ed2k_nodes_dat_url,
            "https://upd.emule-security.org/nodes.dat"
        );
    }
}
