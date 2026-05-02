//! Audio diagnostics for Firewheel nodes.

use bevy_app::prelude::*;
use bevy_diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy_ecs::prelude::*;
use firewheel::processor::ProfilingData;

use crate::{SeedlingSystems, context::AudioContext};

/// Enables audio diagnostic collection.
#[derive(Debug, Default)]
pub struct AudioDiagnosticsPlugin;

impl AudioDiagnosticsPlugin {
    /// Records the total CPU usage of all real-time audio processing.
    pub const AUDIO_BLOCK: DiagnosticPath = DiagnosticPath::const_new("audio_block");
}

impl Plugin for AudioDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.register_diagnostic(Diagnostic::new(Self::AUDIO_BLOCK).with_suffix("%"))
            .init_resource::<AudioProfilingData>()
            .add_systems(Last, diagnostic_system.after(SeedlingSystems::Flush));
    }
}

/// Firewheel's raw profiling data.
///
/// This is updated at most once per frame, though may have
/// slower updates depending on the rate of audio processing.
#[derive(Resource, Default, Debug)]
pub struct AudioProfilingData(pub ProfilingData);

fn diagnostic_system(
    mut diagnostics: Diagnostics,
    mut data: ResMut<AudioProfilingData>,
    mut context: AudioContext,
) {
    context.with(|context| {
        let new_data = context.profiling_data();

        if new_data.version != data.0.version {
            data.0 = new_data.clone();
            diagnostics.add_measurement(&AudioDiagnosticsPlugin::AUDIO_BLOCK, || {
                data.0.overall_cpu_usage * 100.0
            });
        }
    });
}
