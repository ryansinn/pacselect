mod aur;
mod classify;
mod config;
mod config_upgrade;
mod depcheck;
mod environment;
mod filters;
mod install;
mod log;
mod updates;

use anyhow::Result;
use classify::SkipReason;
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::PathBuf;
use updates::PackageUpdate;

#[derive(Parser)]
#[command(
    name = "pacselect",
    about = "Selective pacman updater — updates safe apps, skips system/core and KDE session packages",
    long_about = None,
    version,
)]
struct Cli {
    /// Show what would be updated without installing anything
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Skip the confirmation prompt and install immediately
    #[arg(short = 'y', long)]
    yes: bool,

    /// Don't filter KDE core session packages
    #[arg(long)]
    no_kde_filter: bool,

    /// Don't filter system/core packages (use with caution)
    #[arg(long)]
    no_system_filter: bool,

    /// Extra package pattern to always skip; repeatable, supports globs ("pkg*")
    #[arg(long = "skip", value_name = "PATTERN", action = clap::ArgAction::Append)]
    extra_skip: Vec<String>,

    /// Path to config file [default: ~/.config/pacselect/config.toml]
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Show per-package classification details
    #[arg(short, long)]
    verbose: bool,

    /// Don't show package descriptions in the update list
    #[arg(long)]
    no_descriptions: bool,

    /// Output machine-readable JSON instead of human-readable text.
    /// Implies --dry-run (nothing is installed).
    #[arg(long)]
    json: bool,

    /// Print all built-in filter patterns and exit
    #[arg(long)]
    list_filters: bool,

    /// Print a sample config file to stdout and exit
    #[arg(long)]
    gen_config: bool,

    /// Upgrade the config file with any missing keys (new defaults), preserving
    /// all values you have already set
    #[arg(long)]
    upgrade_config: bool,

    /// Bypass all filters and run a full system upgrade (pacman -Syu).
    /// Use this periodically to apply deferred system/core and KDE updates.
    /// This is the Arch-recommended upgrade path and avoids partial upgrades.
    #[arg(long)]
    full_upgrade: bool,

    /// Don't check for a pacselect update on startup
    #[arg(long)]
    no_self_update: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.gen_config {
        print!("{}", config::sample_config());
        return Ok(());
    }

    // Resolve config path early so --upgrade-config can use it
    let config_path = cli.config.clone().unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("pacselect")
            .join("config.toml")
    });

    if cli.upgrade_config {
        let before = std::fs::read_to_string(&config_path).unwrap_or_default();
        let changed = config_upgrade::upgrade(&config_path, config::sample_config())?;
        if changed {
            let added = config_upgrade::added_keys(config::sample_config(), &before);
            println!("{}", "Config upgraded:".green().bold());
            for key in &added {
                println!("  {} {}", "+".green(), key.dimmed());
            }
            println!("  {}", config_path.display().to_string().dimmed());
        } else {
            println!("{}", "Config is already up to date.".green());
        }
        return Ok(());
    }

    if cli.list_filters {
        print_filter_list();
        return Ok(());
    }

    print_logo();

    // ── Detect running environment ────────────────────────────────────────────
    let env = environment::detect();
    {
        let de = env.desktop.as_deref().unwrap_or("unknown");
        match &env.kde_frameworks_minor {
            Some(minor) => println!(
                "  {} {}  ·  KDE Frameworks {}",
                "Desktop:".dimmed(),
                de.bold(),
                minor.cyan().bold()
            ),
            None => println!("  {} {}", "Desktop:".dimmed(), de.bold()),
        }
    }
    println!();

    // ── Configuration ────────────────────────────────────────────────────────
    let cfg = config::Config::load(&config_path)?;

    // Merge CLI --skip flags with config extra_skip
    let mut extra_skip = cli.extra_skip.clone();
    extra_skip.extend(cfg.filters.extra_skip.iter().cloned());

    let filter_system = !cli.no_system_filter && cfg.filter_sets.system_core;
    let filter_kde = !cli.no_kde_filter && cfg.filter_sets.kde_core;

    // CLI flags take precedence over config defaults
    let show_descriptions = !cli.no_descriptions && cfg.display.descriptions;
    let verbose = cli.verbose || cfg.display.verbose;
    let dry_run = cli.dry_run || cfg.behavior.dry_run;
    let yes = cli.yes || cfg.behavior.auto_confirm;
    let self_update_check = !cli.no_self_update && cfg.behavior.self_update_check;

    // ── Full upgrade path ────────────────────────────────────────────────────
    if cli.full_upgrade {
        println!(
            "{}",
            "Full upgrade mode — all filters disabled (pacman -Syu)."
                .cyan()
                .bold()
        );
        if !yes {
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                println!("{}", "Aborted.".red());
                return Ok(());
            }
        }
        install::full_upgrade()?;
        println!(
            "\n{}",
            "✓ Full system upgrade complete.".green().bold()
        );
        return Ok(());
    }

    // ── Fetch pending updates ────────────────────────────────────────────────
    println!("{}", "Checking for updates...".cyan().bold());

    let pending = updates::get_pending_updates()?;
    let aur_helper = pending.aur_helper;

    if pending.all.is_empty() {
        println!("{}", "✓ System is up to date.".green());
        return Ok(());
    }

    println!(
        "{} {} pending update(s) found",
        "→".cyan(),
        pending.all.len().to_string().bold()
    );

    if verbose {
        println!();
    }

    // ── Detect AUR / foreign packages ───────────────────────────────────────
    let foreign = aur::foreign_packages();

    // ── Self-update check ────────────────────────────────────────────────────
    if self_update_check {
        if let Some(u) = pending.all.iter().find(|u| u.name == "pacselect") {
            if pending.aur_names.contains("pacselect") {
                println!(
                    "  {} {}",
                    "⚠".yellow().bold(),
                    format!(
                        "pacselect has an update available ({} → {})",
                        u.old_version, u.new_version
                    )
                    .yellow()
                    .bold()
                );
                println!(
                    "  {}",
                    "Running an outdated version may produce incorrect filter decisions."
                        .dimmed()
                );
                match aur_helper {
                    Some(h) => {
                        print!("\n  {}", "Update pacselect now? [y/N] ".bold());
                        io::stdout().flush()?;
                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        if matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                            install::self_update_via_helper(h)?;
                            println!(
                                "\n{}  {}",
                                "✓ pacselect updated.".green().bold(),
                                "Re-run pacselect to apply your pending updates.".dimmed()
                            );
                            return Ok(());
                        }
                    }
                    None => println!(
                        "  {}",
                        "No AUR helper found — update manually: paru -S pacselect"
                            .dimmed()
                    ),
                }
                println!();
            }
        }
    }

    // ── Classify ────────────────────────────────────────────────────────────
    let mut safe: Vec<&PackageUpdate> = Vec::new();
    let mut skipped: Vec<(&PackageUpdate, SkipReason)> = Vec::new();

    for update in &pending.all {
        match classify::classify(
            &update.name,
            &update.old_version,
            &update.new_version,
            &extra_skip,
            filter_system,
            filter_kde,
            env.kde_frameworks_minor.as_deref(),
        ) {
            Some(reason) => {
                if verbose && !cli.json {
                    let aur_tag = if foreign.contains(&update.name.to_lowercase()) {
                        " [AUR]".dimmed().to_string()
                    } else {
                        String::new()
                    };
                    println!(
                        "  {} {:<35} {} → {}{}",
                        "SKIP".yellow().bold(),
                        update.name.yellow(),
                        update.old_version.dimmed(),
                        update.new_version.dimmed(),
                        aur_tag,
                    );
                    println!("       {}", format!("({})", reason).dimmed());
                }
                skipped.push((update, reason));
            }
            None => {
                if verbose && !cli.json {
                    let aur_tag = if foreign.contains(&update.name.to_lowercase()) {
                        " [AUR]".dimmed().to_string()
                    } else {
                        String::new()
                    };
                    println!(
                        "  {} {:<35} {} → {}{}",
                        "SAFE".green().bold(),
                        update.name.green(),
                        update.old_version.dimmed(),
                        update.new_version.cyan(),
                        aur_tag,
                    );
                }
                safe.push(update);
            }
        }
    }

    // ── Dependency safety check ──────────────────────────────────────────────
    let skipped_names: HashSet<String> = skipped
        .iter()
        .map(|(u, _)| u.name.to_lowercase())
        .collect();
    let safe_names: Vec<&str> = safe.iter().map(|u| u.name.as_str()).collect();

    // Sync the package DB exactly once here — before both the dep-check query
    // (pacman -Si) and the install step.  Placing the sync here, rather than
    // inside install_packages, prevents the partial-upgrade anti-pattern:
    // a `-Sy` inside the install command would re-sync the DB mid-flight and
    // could let pacman pull in newer library versions that break packages we
    // have intentionally deferred.
    // See: https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported
    depcheck::sync_db();
    let si = depcheck::check_all(&safe_names, &skipped_names);
    let dep_warnings = depcheck::warnings_map(si.dep_warnings);
    let group_demotions = si.group_demotions;
    let descriptions = si.descriptions;
    let reverse_dep_warnings = si.reverse_dep_warnings;
    let all_deps = si.all_deps;

    // Move any safe package whose pacman group implies system/graphics/session
    // membership into skipped.  This catches packages the name patterns missed.
    {
        let mut i = 0;
        while i < safe.len() {
            if let Some(group) = group_demotions.get(safe[i].name.as_str()) {
                let update = safe.remove(i);
                if verbose && !cli.json {
                    println!(
                        "  {} {:<35} {} → {}",
                        "SKIP".yellow().bold(),
                        update.name.yellow(),
                        update.old_version.dimmed(),
                        update.new_version.dimmed(),
                    );
                    println!(
                        "       {}",
                        format!("(pacman group: {})", group).dimmed()
                    );
                }
                skipped.push((update, SkipReason::GroupFilter(group.clone())));
            } else {
                i += 1;
            }
        }
    }

    // Move any safe package that depends on a skipped package into skipped.
    // Installing it alone would be a partial upgrade — block it entirely.
    //
    // This is run as a fixpoint loop: each demotion may expose new transitive
    // partial-upgrade violations (e.g. boost-libs gets demoted because it
    // depends on glibc; snapper depends on boost-libs and must also be demoted
    // even though boost-libs was still safe when the initial dep-warning scan
    // ran).  We iterate using the full all_deps map cached from the -Si query
    // until no further demotions occur.
    {
        // Seed from the first-pass dep_warnings computed against the initial
        // skipped set.
        let mut i = 0;
        while i < safe.len() {
            if let Some(needs) = dep_warnings.get(safe[i].name.as_str()) {
                let update = safe.remove(i);
                if verbose && !cli.json {
                    println!(
                        "  {} {:<35} {} → {}",
                        "SKIP".yellow().bold(),
                        update.name.yellow(),
                        update.old_version.dimmed(),
                        update.new_version.dimmed(),
                    );
                    println!(
                        "       {}",
                        format!("(partial upgrade risk — needs skipped: {})", needs.join(", ")).dimmed()
                    );
                }
                skipped.push((update, SkipReason::PartialUpgrade { needs: needs.clone() }));
            } else {
                i += 1;
            }
        }

        // Transitive fixpoint: keep demoting until nothing changes.
        loop {
            let current_skipped: HashSet<String> = skipped
                .iter()
                .map(|(u, _)| u.name.to_lowercase())
                .collect();

            let mut moved = false;
            let mut i = 0;
            while i < safe.len() {
                let pkg = safe[i].name.to_lowercase();
                if let Some(deps) = all_deps.get(safe[i].name.as_str()) {
                    let needs: Vec<String> = deps
                        .iter()
                        .filter(|d| current_skipped.contains(d.as_str()))
                        .cloned()
                        .collect();
                    if !needs.is_empty() {
                        let update = safe.remove(i);
                        if verbose && !cli.json {
                            println!(
                                "  {} {:<35} {} → {}",
                                "SKIP".yellow().bold(),
                                update.name.yellow(),
                                update.old_version.dimmed(),
                                update.new_version.dimmed(),
                            );
                            println!(
                                "       {}",
                                format!(
                                    "(partial upgrade risk — needs skipped: {})",
                                    needs.join(", ")
                                )
                                .dimmed()
                            );
                        }
                        skipped.push((update, SkipReason::PartialUpgrade { needs }));
                        moved = true;
                        continue;
                    }
                }
                let _ = pkg; // suppress unused warning
                i += 1;
            }

            if !moved {
                break;
            }
        }
    }

    // Block libraries whose soname bumped but whose installed reverse-dependents
    // have no pending update available.  Updating such a library now would break
    // those packages with no fix available until the maintainer releases a rebuild.
    //
    // Libraries where ALL reverse-deps DO have pending updates are advisory-only:
    // a full upgrade will resolve them, so we warn but don't block.

    // Block packages that are exact-version-pinned by an installed package not
    // being updated this run.  This check must run AFTER all other demotions so
    // that the current safe set reflects reality.  pacman enforces exact-version
    // constraints (`pkg=X`) strictly and aborts the transaction if violated.
    {
        let final_safe_names: Vec<&str> = safe.iter().map(|u| u.name.as_str()).collect();
        let final_safe_set: HashSet<String> =
            safe.iter().map(|u| u.name.to_lowercase()).collect();
        let exact_pin_warnings =
            depcheck::check_exact_version_pins(&final_safe_names, &final_safe_set);

        let mut i = 0;
        while i < safe.len() {
            if let Some(w) = exact_pin_warnings
                .iter()
                .find(|w| w.updated_pkg.eq_ignore_ascii_case(&safe[i].name))
            {
                let update = safe.remove(i);
                let pinned_by = w.broken_by.clone();
                if verbose && !cli.json {
                    println!(
                        "  {} {:<35} {} → {}",
                        "SKIP".yellow().bold(),
                        update.name.yellow(),
                        update.old_version.dimmed(),
                        update.new_version.dimmed(),
                    );
                    println!(
                        "       {}",
                        format!(
                            "(exact version pinned by skipped: {})",
                            pinned_by.join(", ")
                        )
                        .dimmed()
                    );
                }
                skipped.push((update, SkipReason::ExactVersionPin { pinned_by }));
            } else {
                i += 1;
            }
        }
    }

    let all_pending_names: HashSet<String> = pending.all.iter()
        .map(|u| u.name.to_lowercase())
        .collect();

    // advisory_rev_warnings: soname bumped, but every broken_by has a pending update
    let mut advisory_rev_warnings: Vec<&depcheck::ReverseDepWarning> = Vec::new();

    for w in &reverse_dep_warnings {
        let missing_rebuilds: Vec<String> = w.broken_by.iter()
            .filter(|r| !all_pending_names.contains(&r.to_lowercase()))
            .cloned()
            .collect();

        if missing_rebuilds.is_empty() {
            // All reverse-deps have pending updates — advisory only.
            advisory_rev_warnings.push(w);
        } else {
            // Some reverse-deps have no pending update — block the library.
            if let Some(pos) = safe.iter().position(|u| u.name.eq_ignore_ascii_case(&w.updated_pkg)) {
                let update = safe.remove(pos);
                if verbose && !cli.json {
                    println!(
                        "  {} {:<35} {} → {}",
                        "SKIP".yellow().bold(),
                        update.name.yellow(),
                        update.old_version.dimmed(),
                        update.new_version.dimmed(),
                    );
                    println!(
                        "       {}",
                        format!("(soname bump — no rebuild yet for: {}  — wait for repo to catch up)", missing_rebuilds.join(", ")).dimmed()
                    );
                }
                skipped.push((update, SkipReason::SonameBump { missing_rebuilds }));
            }
        }
    }

    // ── JSON output path ─────────────────────────────────────────────────────
    if cli.json {
        print_json(
            &env,
            &safe,
            &skipped,
            &foreign,
        );
        // JSON implies dry-run — log it and exit
        let updated_names: Vec<&str> = vec![];
        let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        let _ = log::write_run(&updated_names, &skipped_names_vec, false, true);
        return Ok(());
    }

    // ── Summary bar ─────────────────────────────────────────────────────────
    let n_sys = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::SystemCore | SkipReason::GroupFilter(_)))
        .count();
    let n_gfx = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::Graphics))
        .count();
    let n_kde = skipped
        .iter()
        .filter(|(_, r)| {
            matches!(
                r,
                SkipReason::KdeCore | SkipReason::KdeVersionBump { .. }
            )
        })
        .count();
    let n_usr = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::UserFilter(_)))
        .count();
    let n_partial = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::PartialUpgrade { .. }))
        .count();
    let n_soname = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::SonameBump { .. }))
        .count();
    let n_pin = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::ExactVersionPin { .. }))
        .count();

    println!();
    let bar = "─".repeat(62);
    println!("{}", bar.dimmed());

    // Build a compact summary like:  system: 4  graphics: 2  mesa: 2  kde: 3
    let mut parts: Vec<String> = Vec::new();
    if n_sys   > 0 { parts.push(format!("system: {}",   n_sys));   }
    if n_gfx   > 0 { parts.push(format!("graphics: {}", n_gfx));  }
    if n_kde   > 0 { parts.push(format!("kde: {}",      n_kde));   }
    if n_usr   > 0 { parts.push(format!("user: {}",     n_usr));   }
    if n_partial > 0 { parts.push(format!("partial: {}", n_partial)); }
    if n_soname  > 0 { parts.push(format!("soname: {}" , n_soname));  }
    if n_pin     > 0 { parts.push(format!("pinned: {}",  n_pin));     }    let skipped_detail = if parts.is_empty() {
        String::new()
    } else {
        format!("({})", parts.join("  "))
    };

    println!(
        "  {}  {}    {} skipped  {}",
        "Safe to install:".green().bold(),
        safe.len().to_string().green().bold(),
        skipped.len().to_string().yellow(),
        skipped_detail.dimmed()
    );
    println!("{}", bar.dimmed());

    if safe.is_empty() {
        let critical_pending: Vec<&str> = {
            const CRITICAL_ABI: &[&str] = &[
                "glibc", "lib32-glibc", "gcc-libs", "lib32-gcc-libs",
                "openssl", "nss", "nspr",
                "systemd", "systemd-libs",
                "dbus", "dbus-broker",
                "pam", "linux-pam", "krb5",
                "icu", "zlib", "zstd",
            ];
            skipped
                .iter()
                .filter(|(_, r)| matches!(r, SkipReason::SystemCore))
                .map(|(u, _)| u.name.as_str())
                .filter(|name| CRITICAL_ABI.contains(name))
                .collect()
        };

        if critical_pending.is_empty() {
            println!(
                "\n{}\n  {}",
                "No safe application updates available.".yellow(),
                format!(
                    "When ready, run 'pacselect --full-upgrade' or 'sudo pacman -Syu' \
                     to apply the {} deferred package(s) above.",
                    skipped.len()
                )
                .dimmed()
            );
        } else {
            println!(
                "\n{}\n\n  {} {}\n  {}\n  {}",
                "No safe application updates available.".yellow(),
                "⚠".yellow().bold(),
                "Critical system libraries require a full upgrade:".yellow().bold(),
                critical_pending.join("  ").yellow(),
                "Run 'sudo pacman -Syu' or 'pacselect --full-upgrade' soon."
                    .yellow()
                    .bold()
            );
        }

        let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        let _ = log::write_run(&[], &skipped_names_vec, false, false);
        return Ok(());
    }

    // ── Package list ─────────────────────────────────────────────────────────
    // Partition safe into official repo packages and AUR packages.
    let safe_official: Vec<&PackageUpdate> = safe.iter().copied()
        .filter(|u| !pending.aur_names.contains(&u.name))
        .collect();
    let safe_aur: Vec<&PackageUpdate> = safe.iter().copied()
        .filter(|u| pending.aur_names.contains(&u.name))
        .collect();

    println!("\n{}", "Packages that will be updated:".bold());
    for u in &safe_official {
        println!(
            "  {:<35} {} → {}",
            u.name.green(),
            u.old_version.dimmed(),
            u.new_version.cyan(),
        );
        if let Some(desc) = descriptions.get(u.name.as_str()) {
            if show_descriptions {
                println!("     {}", desc.dimmed());
            }
        }
    }

    if !safe_aur.is_empty() {
        println!("\n  {}", "── AUR packages ──".bold());
        for u in &safe_aur {
            println!(
                "  {:<35} {} → {}",
                u.name.cyan(),
                u.old_version.dimmed(),
                u.new_version.cyan(),
            );
        }
        let aur_install_hint = match aur_helper {
            Some(h) => format!("run '{}' to update these", h),
            None    => "no AUR helper found — update these manually".to_string(),
        };
        println!("  {}", aur_install_hint.dimmed());
    }

    // Show skipped names compactly grouped by reason when not in verbose mode
    if !skipped.is_empty() && !verbose {
        println!(
            "\n{} ({}) — use {} for details:",
            "Skipped".yellow().bold(),
            skipped.len(),
            "--verbose".bold()
        );
        print_skipped_grouped(&skipped);
    }

    // ── Reverse-dependency ABI warnings ─────────────────────────────────────
    // Advisory: soname bumped, but all reverse-deps have pending rebuilds in
    // the repo — safe to full-upgrade, everything lands together.
    // Blocked libraries (some reverse-deps have no rebuild yet) are already
    // in the skipped list with SkipReason::SonameBump.
    if !advisory_rev_warnings.is_empty() && !cli.json {
        println!();
        println!(
            "  {} {}",
            "ℹ".cyan().bold(),
            "Soname bump — these packages will update alongside this library:"
                .cyan()
                .bold()
        );
        println!(
            "  {}",
            "(All reverse-dependents have rebuilds pending — safe to run --full-upgrade)"
                .dimmed()
        );
        println!();
        for w in &advisory_rev_warnings {
            println!("  {} {}", w.updated_pkg.cyan().bold(), "─".repeat(58usize.saturating_sub(w.updated_pkg.len())).dimmed());
            let indent = "    ";
            let max_width = 76usize;
            let mut col = indent.len();
            print!("{}", indent);
            for name in &w.broken_by {
                let token = format!("{}  ", name);
                if col + token.len() > max_width && col > indent.len() {
                    println!();
                    print!("{}", indent);
                    col = indent.len();
                }
                print!("{}", name.dimmed());
                print!("  ");
                col += token.len();
            }
            println!();
        }
    }

    if dry_run {
        println!("\n{}", "[ Dry run — nothing installed ]".cyan().italic());
        let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        let _ = log::write_run(&[], &skipped_names_vec, false, true);
        return Ok(());
    }

    // ── Confirmation ─────────────────────────────────────────────────────────
    if !yes {
        print!("\n{}", "Proceed with installation? [y/N] ".bold());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("{}", "Aborted.".red());
            let skipped_names_vec: Vec<&str> =
                skipped.iter().map(|(u, _)| u.name.as_str()).collect();
            let _ = log::write_run(&[], &skipped_names_vec, true, false);
            return Ok(());
        }
    }

    // ── Install ──────────────────────────────────────────────────────────────
    // AUR packages cannot be installed via pacman; install official ones only.
    let package_names: Vec<&str> = safe_official.iter().map(|u| u.name.as_str()).collect();
    install::install_packages(&package_names)?;

    let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
    let _ = log::write_run(&package_names, &skipped_names_vec, false, false);

    println!(
        "\n{} {} package(s) updated.",
        "✓".green().bold(),
        package_names.len()
    );

    if !safe_aur.is_empty() {
        let aur_pkg_names: Vec<&str> = safe_aur.iter().map(|u| u.name.as_str()).collect();
        match aur_helper {
            Some(h) => println!(
                "  {} {} AUR package(s) — run '{}' to update: {}",
                "→".cyan(),
                aur_pkg_names.len(),
                h,
                aur_pkg_names.join("  ").cyan()
            ),
            None => println!(
                "  {} {} AUR package(s) need manual update (no AUR helper found): {}",
                "→".cyan(),
                aur_pkg_names.len(),
                aur_pkg_names.join("  ").cyan()
            ),
        }
    }

    // Nudge the user toward a full upgrade when system/core packages are
    // accumulating.  Partial upgrades become riskier the longer they drift.
    if n_sys + n_kde + n_gfx > 0 {
        print_critical_abi_warning(&skipped);
    }

    Ok(())
}

fn print_skipped_grouped(skipped: &[(&updates::PackageUpdate, SkipReason)]) {
    // Each group: (label, filter closure)
    struct Group {
        label: &'static str,
        names: Vec<String>,
    }

    let mut groups: Vec<Group> = vec![
        Group { label: "system",   names: Vec::new() },
        Group { label: "graphics", names: Vec::new() },
        Group { label: "kde",      names: Vec::new() },
        Group { label: "user",     names: Vec::new() },
        Group { label: "partial",  names: Vec::new() },
        Group { label: "soname",   names: Vec::new() },
        Group { label: "pinned",   names: Vec::new() },
    ];

    for (u, reason) in skipped {
        let idx = match reason {
            SkipReason::SystemCore
            | SkipReason::GroupFilter(_)        => 0,
            SkipReason::Graphics                => 1,
            SkipReason::KdeCore
            | SkipReason::KdeVersionBump { .. } => 2,
            SkipReason::UserFilter(_)           => 3,
            SkipReason::PartialUpgrade { .. }   => 4,
            SkipReason::SonameBump { .. }       => 5,
            SkipReason::ExactVersionPin { .. }  => 6,
        };
        groups[idx].names.push(u.name.clone());
    }

    for g in &groups {
        if g.names.is_empty() { continue; }
        // Label column is 10 chars wide so rows stay aligned
        print!("  {:<10}", format!("{}:", g.label).yellow().bold());
        let mut col = 12usize; // 2 indent + 10 label
        for name in &g.names {
            let token = format!("{}  ", name);
            if col + token.len() > 78 {
                println!();
                print!("  {:<10}", "");
                col = 12;
            }
            print!("{}", token.dimmed());
            col += token.len();
        }
        println!();
    }
}

fn print_critical_abi_warning(skipped: &[(&updates::PackageUpdate, SkipReason)]) {
    // Packages whose ABI or runtime is shared by many running processes.
    // Deferring these for more than a few days meaningfully increases the
    // risk of a broken system state.
    const CRITICAL_ABI: &[&str] = &[
        "glibc", "lib32-glibc", "gcc-libs", "lib32-gcc-libs",
        "openssl", "nss", "nspr",
        "systemd", "systemd-libs",
        "dbus", "dbus-broker",
        "pam", "linux-pam", "krb5",
        "icu", "zlib", "zstd",
    ];

    let n_sys_kde = skipped
        .iter()
        .filter(|(_, r)| matches!(
            r,
            SkipReason::SystemCore
                | SkipReason::GroupFilter(_)
                | SkipReason::Graphics
                | SkipReason::KdeCore
                | SkipReason::KdeVersionBump { .. }
        ))
        .count();

    let critical_pending: Vec<&str> = skipped
        .iter()
        .filter(|(_, r)| matches!(r, SkipReason::SystemCore))
        .map(|(u, _)| u.name.as_str())
        .filter(|name| CRITICAL_ABI.contains(name))
        .collect();

    if critical_pending.is_empty() {
        println!(
            "  {}",
            format!(
                "Note: {} deferred system/core package(s) pending — run \
                 'pacselect --full-upgrade' or 'sudo pacman -Syu' periodically \
                 to avoid partial-upgrade drift.",
                n_sys_kde
            )
            .dimmed()
        );
    } else {
        println!(
            "\n  {} {}",
            "⚠".yellow().bold(),
            "Critical system libraries are pending a full upgrade:".yellow().bold()
        );
        println!("  {}", critical_pending.join("  ").yellow());
        println!(
            "  {}",
            "These packages are shared by many running processes. Leaving them \
             out of sync with the rest of the system increases the risk of \
             instability or broken dependencies."
                .yellow()
        );
        println!(
            "  {}",
            "Run 'sudo pacman -Syu' or 'pacselect --full-upgrade' soon."
                .yellow()
                .bold()
        );
    }
}

// ── JSON output types ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonOutput<'a> {
    desktop: Option<&'a str>,
    kde_frameworks: Option<&'a str>,
    safe: Vec<JsonPackage<'a>>,
    skipped: Vec<JsonSkipped<'a>>,
}

#[derive(Serialize)]
struct JsonPackage<'a> {
    name: &'a str,
    old: &'a str,
    new: &'a str,
    aur: bool,
}

#[derive(Serialize)]
struct JsonSkipped<'a> {
    name: &'a str,
    old: &'a str,
    new: &'a str,
    reason: String,
    aur: bool,
}

fn print_json(
    env: &environment::SystemEnv,
    safe: &[&PackageUpdate],
    skipped: &[(&PackageUpdate, SkipReason)],
    foreign: &std::collections::HashSet<String>,
) {
    let safe_json: Vec<JsonPackage> = safe
        .iter()
        .map(|u| JsonPackage {
            name: &u.name,
            old: &u.old_version,
            new: &u.new_version,
            aur: foreign.contains(&u.name.to_lowercase()),
        })
        .collect();

    let skipped_json: Vec<JsonSkipped> = skipped
        .iter()
        .map(|(u, r)| JsonSkipped {
            name: &u.name,
            old: &u.old_version,
            new: &u.new_version,
            reason: r.to_string(),
            aur: foreign.contains(&u.name.to_lowercase()),
        })
        .collect();

    let out = JsonOutput {
        desktop: env.desktop.as_deref(),
        kde_frameworks: env.kde_frameworks_minor.as_deref(),
        safe: safe_json,
        skipped: skipped_json,
    };

    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

fn print_logo() {
    // Figlet "standard" font, letters combined side-by-side.
    let pac = [
        " ____       _    ____  ",
        "|  _ \\     / \\  / ___|",
        "| |_) |   / _ \\| |    ",
        "|  __/   / ___ \\ |___ ",
        "|_|     /_/   \\_\\____|",
    ];
    let select = [
        "  ____       _           _   ",
        " / ___|  ___| | ___  ___| |_ ",
        " \\___ \\ / _ \\ |/ _ \\/ __| __|",
        "  ___) |  __/ |  __/ (__| |_ ",
        " |____/ \\___|_|\\___|\\___|\\__|",
    ];
    println!();
    for (p, s) in pac.iter().zip(select.iter()) {
        println!("{}{}", p.white().bold(), s.red().bold());
    }
    println!(
        "{}",
        "  --- Smart app updates. Stable system. ---".dimmed()
    );
    println!();
}

fn print_filter_list() {
    println!("{}", "System/Core filter patterns:".bold());
    for p in filters::SYSTEM_CORE_PATTERNS {
        println!("  {}", p);
    }
    println!();
    println!("{}", "Graphics filter patterns:".bold());
    for p in filters::GRAPHICS_PATTERNS {
        println!("  {}", p);
    }
    println!();
    println!("{}", "KDE Core filter patterns:".bold());
    for p in filters::KDE_CORE_PATTERNS {
        println!("  {}", p);
    }
    println!();
    println!("{}", "KDE Ecosystem patterns (version-bump detection):".bold());
    for p in filters::KDE_ECOSYSTEM_PATTERNS {
        println!("  {}", p);
    }
}
