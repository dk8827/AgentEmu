//! Minimum frame-execution boundary. Game adapters intentionally remain system
//! specific; CPU registers, RAM maps, mappers and pixel formats do not belong here.
//! A future console supplies its own input and checkpoint types.
pub trait Console {
    type Input;
    type Checkpoint;
    type Error: std::fmt::Display;

    /// Execute one video frame. Fatal faults must remain latched until reset or
    /// successful restore. Audio returned by `take_audio` belongs to this frame.
    fn step(&mut self, input: Self::Input) -> Result<u64, Self::Error>;
    fn take_audio(&mut self) -> Vec<f32>;
    fn checkpoint(&self) -> Self::Checkpoint;
    fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<(), Self::Error>;
}

pub struct Frame {
    pub cycles: u64,
    pub audio: Vec<f32>,
}

/// Used after system-specific input arbitration by interactive, replay and
/// training runners. No host may independently step the CPU to render a frame.
pub fn advance<C: Console>(console: &mut C, input: C::Input) -> Result<Frame, String> {
    let cycles = console.step(input).map_err(|e| e.to_string())?;
    Ok(Frame {
        cycles,
        audio: console.take_audio(),
    })
}

impl Console for nes_core::machine::Machine {
    type Input = [u8; 2];
    type Checkpoint = nes_core::machine::Checkpoint;
    type Error = nes_core::machine::MachineError;
    fn step(&mut self, input: Self::Input) -> Result<u64, Self::Error> {
        // Avoid even changing controller latches when the machine is faulted.
        if self.fault().is_some() {
            return self.run_frame();
        }
        self.set_controllers(input);
        self.run_frame()
    }
    fn take_audio(&mut self) -> Vec<f32> {
        self.take_audio()
    }
    fn checkpoint(&self) -> Self::Checkpoint {
        self.checkpoint()
    }
    fn restore(&mut self, state: &Self::Checkpoint) -> Result<(), Self::Error> {
        self.restore_checkpoint(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_execution_has_no_nes_input_or_state_assumptions() {
        struct Other {
            position: i32,
        }
        impl Console for Other {
            type Input = i32;
            type Checkpoint = i32;
            type Error = &'static str;
            fn step(&mut self, input: i32) -> Result<u64, Self::Error> {
                if input == 99 {
                    return Err("fault");
                }
                self.position += input;
                Ok(123)
            }
            fn take_audio(&mut self) -> Vec<f32> {
                vec![self.position as f32]
            }
            fn checkpoint(&self) -> i32 {
                self.position
            }
            fn restore(&mut self, state: &i32) -> Result<(), Self::Error> {
                self.position = *state;
                Ok(())
            }
        }
        let mut other = Other { position: 0 };
        let saved = other.checkpoint();
        let frame = advance(&mut other, 3).unwrap();
        assert_eq!(frame.cycles, 123);
        assert_eq!(frame.audio, [3.]);
        assert!(advance(&mut other, 99).is_err());
        other.restore(&saved).unwrap();
        assert_eq!(advance(&mut other, -2).unwrap().audio, [-2.]);
    }
}
