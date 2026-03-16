mod aur;
mod classify;
mod config;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.gen_config {
        print!("{}", config::sample_config());
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
    let config_path = cli.config.clone().unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("pacselect")
            .join("config.toml")
    });

    let cfg = config::Config::load(&config_path)?;

    // Merge CLI --skip flags with config extra_skip
    let mut extra_skip = cli.extra_skip.clone();
    extra_skip.extend(cfg.filters.extra_skip.iter().cloned());

    let filter_system = !cli.no_system_filter && cfg.filter_sets.system_core;
    let filter_kde = !cli.no_kde_filter && cfg.filter_sets.kde_core;

    // ── Fetch pending updates ────────────────────────────────────────────────
    println!("{}", "Checking for updates...".cyan().bold());

    let pending = updates::get_pending_updates()?;

    if pending.is_empty() {
        println!("{}", "✓ System is up to date.".green());
        return Ok(());
    }

    println!(
        "{} {} pending update(s) found",
        "→".cyan(),
        pending.len().to_string().bold()
    );

    if cli.verbose {
        println!();
    }

    // ── Detect AUR / foreign packages ───────────────────────────────────────
    let foreign = aur::foreign_packages();

    // ── Classify ────────────────────────────────────────────────────────────
    let mut safe: Vec<&PackageUpdate> = Vec::new();
    let mut skipped: Vec<(&PackageUpdate, SkipReason)> = Vec::new();

    for update in &pending {
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
                if cli.verbose && !cli.json {
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
                if cli.verbose && !cli.json {
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
    let dep_warnings = depcheck::warnings_map(depcheck::check(&safe_names, &skipped_names));

    // Move any safe package that depends on a skipped package into skipped.
    // Installing it alone would be a partial upgrade — block it entirely.
    {
        let mut i = 0;
        while i < safe.len() {
            if let Some(needs) = dep_warnings.get(safe[i].name.as_str()) {
                let update = safe.remove(i);
                skipped.push((update, SkipReason::PartialUpgrade { needs: needs.clone() }));
            } else {
                i += 1;
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
        .filter(|(_, r)| matches!(r, SkipReason::SystemCore))
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

    println!();
    let bar = "─".repeat(62);
    println!("{}", bar.dimmed());
    let partial_note = if n_partial > 0 {
        format!("  partial: {}", n_partial)
    } else {
        String::new()
    };
    println!(
        "  {}  {}    {} skipped  {}",
        "Safe to install:".green().bold(),
        safe.len().to_string().green().bold(),
        skipped.len().to_string().yellow(),
        format!("(system: {}  kde: {}  user: {}{})", n_sys, n_kde, n_usr, partial_note).dimmed()
    );
    println!("{}", bar.dimmed());

    if safe.is_empty() {
        println!(
            "\n{}\n{}",
            "No safe application updates available.".yellow(),
            "Run 'sudo pacman -Syu' for a full system upgrade.".dimmed()
        );
        let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        let _ = log::write_run(&[], &skipped_names_vec, false, false);
        return Ok(());
    }

    // ── Package list ─────────────────────────────────────────────────────────
    println!("\n{}", "Packages that will be updated:".bold());
    for u in &safe {
        let aur_tag = if foreign.contains(&u.name.to_lowercase()) {
            format!(" {}", "[AUR]".dimmed())
        } else {
            String::new()
        };
        println!(
            "  {:<35} {} → {}{}",
            u.name.green(),
            u.old_version.dimmed(),
            u.new_version.cyan(),
            aur_tag,
        );
    }

    // Show skipped names compactly when not in verbose mode
    if !skipped.is_empty() && !cli.verbose {
        println!(
            "\n{} ({}) — use {} for details:",
            "Skipped".yellow().bold(),
            skipped.len(),
            "--verbose".bold()
        );
        let names: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        for chunk in names.chunks(6) {
            println!("  {}", chunk.join("  ").yellow().to_string().dimmed());
        }
    }

    if cli.dry_run {
        println!("\n{}", "[ Dry run — nothing installed ]".cyan().italic());
        let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
        let _ = log::write_run(&[], &skipped_names_vec, false, true);
        return Ok(());
    }

    // ── Confirmation ─────────────────────────────────────────────────────────
    if !cli.yes {
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
    let package_names: Vec<&str> = safe.iter().map(|u| u.name.as_str()).collect();
    install::install_packages(&package_names)?;

    let skipped_names_vec: Vec<&str> = skipped.iter().map(|(u, _)| u.name.as_str()).collect();
    let _ = log::write_run(&package_names, &skipped_names_vec, false, false);

    println!(
        "\n{} {} package(s) updated.",
        "✓".green().bold(),
        safe.len()
    );
    Ok(())
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
