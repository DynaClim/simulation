use anyhow::{Context, Error as AnyError, Result, anyhow, ensure};
use argh::FromArgs;
use log::{LevelFilter, error, info, warn};
use logout::new_log;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use integrators::{Error as IntegratorError, Integrator, IntegratorType, Stats, System};
use sci_file::{
    OutputFile, collect_files_from_dir_path, create_directory, create_incremented_directory,
    deserialize_json_from_path, serialize_json_to_path,
};

//#[derive(Parser, Debug)]
//#[command(version, about, long_about = "Simulation controller.")]
#[derive(FromArgs, Debug)]
/// simulation launcher.
struct Cli {
    /// batch mode. Process all config files in the `config` directory.
    #[argh(switch, short = 'b')]
    batch_mode: bool,
    /// path to the input config file. If batch mode is enabled `-b`, a directory containing config files.
    #[argh(positional)]
    input_path: PathBuf,
    /// path to a directory to write output files. Will be created if it doesn't exist.
    #[argh(positional)]
    output_path: PathBuf,
}

/// This defines the structure of the input config file to be deserialized.
#[derive(Deserialize, Serialize, Debug)]
pub struct InputConfig<U> {
    /// Define whether to resume a simulation.
    pub resume: bool,
    pub initial_time: f64,
    pub final_time: f64,
    // Name of integrator.
    pub integrator: IntegratorType,
    /// Contains all data available to the derivation function of the integrator.
    pub universe: U,
}

#[derive(Debug)]
pub struct Simulation<U: Debug, S: System> {
    pub name: String,
    pub resume: bool,
    pub initial_time: f64,
    pub final_time: f64,
    pub integrator: IntegratorType,
    /// Contains all data passed to the derivation function of the integrator and the `OutputFile` for solout output.
    pub system: S,
    /// Placeholder for the generic type `U` that is packaged into the output of `S`. i.e `S::new(T, U)`
    _phantom: PhantomData<U>,
}

impl<U: Serialize + for<'a> Deserialize<'a> + Debug, S: System<Output = OutputFile, Data = U>>
    Simulation<U, S>
{
    pub fn new() -> Result<Vec<Simulation<U, S>>> {
        // Parse arguments.
        let cli: Cli = argh::from_env();

        create_directory(&cli.output_path)?;

        // Initialise the logger.
        let log_path = cli.output_path.join("simulation.log");
        new_log()
            .to_file(&log_path)?
            .max_log_level(LevelFilter::Info)
            .enable()
            .context(format!(
                "unable to open logfile \"{}\".",
                log_path.display()
            ))?;

        // Parse all the input configs into simulations.
        if cli.batch_mode {
            collect_files_from_dir_path(cli.input_path)?
                .iter()
                .filter(|config| config.extension() == Some(OsStr::new("conf")))
                .map(|config| Self::setup(config, &cli.output_path))
                .collect::<Result<Vec<Simulation<U, S>>>>()
        } else {
            let sim = Self::setup(&cli.input_path, &cli.output_path)?;
            Ok(vec![sim])
        }
    }

    fn setup(input_path: &Path, output_path: &Path) -> Result<Simulation<U, S>> {
        // Read the values specified in the input file.
        let config: InputConfig<U> = deserialize_json_from_path(input_path).context(format!(
            "unable to load config from file: {}",
            input_path.display()
        ))?;

        ensure!(
            config.initial_time < config.final_time,
            "Simulation final_time must be greater than initial_time."
        );

        let name = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("unable to create output config")?
            .to_string();

        // Create new output directory for this simulation.
        let mut outpath = create_incremented_directory(output_path)?.join(&name);

        // Copy the input config file to the results directory for reproducibility.
        _ = outpath.set_extension("conf");
        serialize_json_to_path(&config, &outpath)?;

        // Create file to save the output of the simulation.
        _ = outpath.set_extension("jsonl");
        let outfile = OutputFile::new(&outpath)?;

        // Create a new `System` with user specified `Universe` structure and `OutputFile`
        // to pass into the `Integrator`.
        let system = System::new(outfile, config.universe);

        Ok(Self {
            name,
            resume: config.resume,
            initial_time: config.initial_time,
            final_time: config.final_time,
            integrator: config.integrator,
            system,
            _phantom: PhantomData,
        })
    }

    // Launch a single simulation, logging results to the logfile.
    pub fn launch(mut self, x: f64, x_final: f64, y: &[f64]) -> Result<()> {
        // Apply the initial values for a new simulation.
        // For a resume simulation the values will already be in the integrator snapshot.
        if !self.resume {
            self.integrator.initialise(x, x_final, y)?;
        }

        // Run the integration and check the result.
        log_start(&self.name, self.initial_time, self.final_time);
        match self.integrator.integrate(&mut self.system) {
            Ok(stats) => log_success(&self.name, self.final_time, &stats),
            Err(why) => log_failure(&self.name, &anyhow!(why)),
        }

        Ok(())
    }
}

fn log_start(name: &str, initial_time: f64, final_time: f64) {
    info!(
        "Starting simulation {} at time period of {} to {} (total {})",
        name,
        initial_time,
        final_time,
        final_time - initial_time
    );
}

fn log_success(name: &str, final_time: f64, stats: &Stats) {
    info!(
        "Completed simulation {} at time {}. Integrator stats: {}",
        name, final_time, stats,
    );
}

fn log_failure(name: &str, why: &AnyError) {
    match why.downcast_ref() {
        // Integration terminated early due to maximum number of steps reached.
        Some(IntegratorError::StepLimitReached { x: time, n_step }) => warn!(
            "Terminating simulation {} after {} years as maximum {n_step} steps reached.",
            name, time
        ),
        // Other integration error, check the log for specifics.
        _ => log_error_chain(why, format!("Aborting simulation {name} due to failure.")),
    };
}

// Unwinds chains of errors, flattening them into a single log entry.
fn log_error_chain(e: &AnyError, mut s: String) {
    for cause in e.chain() {
        s.push_str(&format!(": {cause}"));
    }
    eprintln!("Error: {}", &s);
    error!("{s}");
}
