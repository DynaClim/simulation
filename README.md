# Simulation

This is a library crate, but is more of a binary.

It handles parsing of an input config file, initialising logging, configuring the integrator, etc for a simulation.

# Example

```rust
use integrators::{Integrator, System};
use simulation::InputConfig;
use std::path::PathBuf;

struct Program {
    pub data: Universe,
    pub output: OutputFile
}

// Mock System implementation without writing any output.
impl System for Test {
    type Data = Universe;
    type Output = OutputFile;

    fn new(output: OutputFile, data: Universe) -> Self {
        Self { data, output }
    }

    // Called by the integrator to solve the ODEs.
    fn derive(
        &mut self,
        time: f64,
        y: &[f64],
        dy: &mut [f64],
    ) -> Result<(), Box<(dyn std::error::Error + Send + Sync + 'static)>> {
        // Call into physics module computing specific derivation functions.
        Ok()
    }

    // Called by the integrator to output intermediate and final solutions.
    fn solout(
        &mut self,
        _time: f64,
        _y: &[f64],
    ) -> Result<(), Box<(dyn std::error::Error + Send + Sync + 'static)>> {
        Ok(())
    }
}

fn main() -> Result<> {

    let mut simulation = Simulation::<Universe, Spiroid>::new()?;
    simulation.initial_time *= SECONDS_IN_YEAR;
    simulation.final_time *= SECONDS_IN_YEAR;

    // Initialise the universe (star, planet, etc).
    simulation.universe.initialise(simulation.initial_time)?;

    // Initial values for the integrator.
    let y = simulation.universe.integration_quantities();
    // y[0] is ...
    // y[1] is ...
    // y[2] is ...

    // Run the full simulation.
    simulation.launch(initial_time, final_time, &y)?;

    // Collect the final y values.
    let dy = simulation.integrator.y_final();
    println!("{:?}", &dy);
}
```

# TODO
Docs
