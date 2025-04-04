use chemistry::{
    add_chemistry, clean_chemistries, fetch_chemistries, lookup_chemistry, refresh_chemistries,
    remove_chemistry,
};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

use anyhow::bail;
use clap::{crate_version, Parser};

use std::env;
use std::path::PathBuf;

mod defaults;
mod utils;

// all of the relevant commands
// live in this module.
mod simpleaf_commands;
use simpleaf_commands::*;
mod atac;

/// simplifying alevin-fry workflows
#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> anyhow::Result<()> {
    // Check the `RUST_LOG` variable for the logger level and
    // respect the value found there. If this environment
    // variable is not set then set the logging level to
    // INFO.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy()
                // we don't want to hear anything below a warning from ureq
                .add_directive("ureq=warn".parse()?),
        )
        .init();

    // Before we do anything else, ensure that the user has
    // their `AF_HOME` variable set in the environment, as we
    // will be using this with mostly every command.
    // TODO: Should, instead of requiring a specific `AF_HOME`
    // we be following the
    // [XDG standard](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
    const AF_HOME: &str = "ALEVIN_FRY_HOME";
    let af_home_path = match env::var(AF_HOME) {
        Ok(p) => PathBuf::from(p),
        Err(e) => {
            bail!(
                "${} is unset {}, please set this environment variable to continue.",
                AF_HOME,
                e
            );
        }
    };

    let cli_args = Cli::parse();

    // Based on the command we parsed, dispatch
    // to the appropriate function.
    match cli_args.command {
        // set the paths where the relevant tools live
        Commands::SetPaths(sp_opts) => set_paths(af_home_path, sp_opts),
        // chemistry related commands
        Commands::Chemistry(ChemistryCommand::Add(add_opts)) => {
            add_chemistry(af_home_path, add_opts)
        }
        Commands::Chemistry(ChemistryCommand::Remove(rem_opts)) => {
            remove_chemistry(af_home_path, rem_opts)
        }
        Commands::Chemistry(ChemistryCommand::Clean(clean_opts)) => {
            clean_chemistries(af_home_path, clean_opts)
        }
        Commands::Chemistry(ChemistryCommand::Lookup(lookup_opts)) => {
            lookup_chemistry(af_home_path, lookup_opts)
        }
        Commands::Chemistry(ChemistryCommand::Refresh(refresh_opts)) => {
            refresh_chemistries(af_home_path, refresh_opts)
        }
        Commands::Chemistry(ChemistryCommand::Fetch(fetch_opts)) => {
            fetch_chemistries(af_home_path, fetch_opts)
        }
        // Inspect the status of simpleaf
        Commands::Inspect {} => inspect_simpleaf(crate_version!(), af_home_path),
        // re-refresh the versions information of all of the programs
        Commands::RefreshProgInfo {} => refresh_prog_info(af_home_path),

        // if we are building the reference and indexing
        Commands::Index(index_opts) => build_ref_and_index(af_home_path.as_path(), index_opts),

        // if we are running mapping and quantification
        Commands::Quant(map_quant_opts) => map_and_quant(af_home_path.as_path(), map_quant_opts),

        // indexing for ATAC-seq data
        Commands::Atac(AtacCommand::Index(index_opts)) => {
            atac::index::piscem_index(af_home_path.as_path(), &index_opts)
        }

        // processing for ATAC-seq data
        Commands::Atac(AtacCommand::Process(process_opts)) => {
            // validate versions
            atac::process::check_progs(&af_home_path, &process_opts)?;
            // first we map the reads
            atac::process::map_reads(af_home_path.as_path(), &process_opts)?;
            // then we generate the permit list and sort the file
            atac::process::gen_bed(af_home_path.as_path(), &process_opts)
        }

        Commands::Workflow(workflow_args) => {
            let workflow_cmd = workflow_args.command;
            match workflow_cmd {
                // if we are running or parsing a
                // workflow file.
                WorkflowCommands::Run {
                    template,
                    output,
                    no_execution,
                    manifest,
                    start_at,
                    resume,
                    jpaths,
                    skip_step,
                    ext_codes,
                } => run_workflow(
                    af_home_path.as_path(),
                    WorkflowCommands::Run {
                        template,
                        output,
                        no_execution,
                        manifest,
                        start_at,
                        resume,
                        jpaths,
                        skip_step,
                        ext_codes,
                    },
                ),

                // if we are generating a workflow
                // configuration from a workflow template.
                WorkflowCommands::Get {
                    output,
                    name,
                    // essential_only,
                } => get_workflow(
                    af_home_path.as_path(),
                    WorkflowCommands::Get {
                        output,
                        name,
                        // essential_only,
                    },
                ),

                WorkflowCommands::Patch {
                    manifest: manifest_opt,
                    template: template_opt,
                    patch,
                    output,
                } => patch_manifest_or_template(
                    af_home_path.as_path(),
                    WorkflowCommands::Patch {
                        manifest: manifest_opt,
                        template: template_opt,
                        patch,
                        output,
                    },
                ),

                WorkflowCommands::List {} => list_workflows(af_home_path.as_path()),
                WorkflowCommands::Refresh {} => refresh_protocol_estuary(af_home_path.as_path()),
            }
        }
    }
    // success, yay!
    // we should not need an explicit value here as the
    // match above is exhaustive, and each command should
    // return an appropriate `Result`.
}
