//! Typed observations cross Python as one little-endian f32 buffer per step.
//! Diagnostic state is converted only if requested; no JSON text on the hot path.
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyBytes, PyDict, PyList},
};
use rayon::prelude::*;
use serde_json::Value;
use session_engine::{episode, tasks};
fn error(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}
enum Info {
    Contra(episode::Info),
    Task(tasks::Info),
}
#[pyclass(frozen)]
pub struct StepData {
    observations: Vec<u8>,
    #[pyo3(get)]
    pub rewards: [f32; 2],
    #[pyo3(get)]
    pub reward: f32,
    #[pyo3(get)]
    pub terminated: bool,
    #[pyo3(get)]
    pub truncated: bool,
    #[pyo3(get)]
    pub skipped_transition_frames: u32,
    info: Info,
}
fn pack(observations: [Vec<f32>; 2]) -> Vec<u8> {
    observations
        .into_iter()
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect()
}
impl From<episode::Step> for StepData {
    fn from(s: episode::Step) -> Self {
        Self {
            observations: pack(s.observations),
            reward: if s.info.player_count == 1 {
                s.rewards[0]
            } else {
                (s.rewards[0] + s.rewards[1]) * 0.5
            },
            rewards: s.rewards,
            terminated: s.terminated,
            truncated: s.truncated,
            skipped_transition_frames: 0,
            info: Info::Contra(s.info),
        }
    }
}
impl From<tasks::Step> for StepData {
    fn from(s: tasks::Step) -> Self {
        Self {
            observations: pack(s.observations),
            reward: s.reward,
            rewards: [s.reward; 2],
            terminated: s.terminated,
            truncated: s.truncated,
            skipped_transition_frames: 0,
            info: Info::Task(s.info),
        }
    }
}
// Preserve the public diagnostic dictionaries, without encoding and parsing JSON.
fn object(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(v) => v.into_pyobject(py)?.to_owned().into_any().unbind(),
        Value::String(v) => v.into_pyobject(py)?.into_any().unbind(),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                n.into_pyobject(py)?.into_any().unbind()
            } else if let Some(n) = v.as_u64() {
                n.into_pyobject(py)?.into_any().unbind()
            } else {
                v.as_f64().unwrap().into_pyobject(py)?.into_any().unbind()
            }
        }
        Value::Array(v) => {
            let list = PyList::empty(py);
            for item in v {
                list.append(object(py, item)?)?;
            }
            list.into_any().unbind()
        }
        Value::Object(v) => {
            let dict = PyDict::new(py);
            for (k, v) in v {
                dict.set_item(k, object(py, v)?)?;
            }
            dict.into_any().unbind()
        }
    })
}
#[pymethods]
impl StepData {
    #[getter]
    fn observations<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.observations)
    }
    #[getter]
    fn info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        object(
            py,
            match &self.info {
                Info::Contra(i) => serde_json::to_value(i),
                Info::Task(i) => serde_json::to_value(i),
            }
            .map_err(error)?,
        )
    }
}
enum Episode {
    Contra(Box<episode::Episode>),
    Task(Box<tasks::Episode>),
}
impl Episode {
    fn reset(&mut self, warmup: u32) -> PyResult<StepData> {
        match self {
            Self::Contra(e) => e.reset_timing(warmup).map(Into::into).map_err(error),
            Self::Task(e) => e.reset(warmup).map(Into::into).map_err(error),
        }
    }
    fn step(&mut self, masks: [u8; 2]) -> PyResult<StepData> {
        match self {
            Self::Contra(e) => e.step(masks).map(Into::into).map_err(error),
            Self::Task(e) => e.step(masks).map(Into::into).map_err(error),
        }
    }
    fn decision_step(&mut self, masks: [u8; 2], skip_transitions: bool) -> PyResult<StepData> {
        let mut out = self.step(masks)?;
        let mut rewards = out.rewards;
        let mut reward = out.reward;
        let mut skipped = 0;
        while skip_transitions && !out.terminated && !out.truncated {
            let gameplay = match &out.info {
                Info::Contra(info) => info.state.gameplay,
                Info::Task(info) => info.state["gameplay"].as_bool().unwrap_or(true),
            };
            if gameplay {
                break;
            }
            out = self.step([0, 0])?;
            reward += out.reward;
            for (total, value) in rewards.iter_mut().zip(out.rewards) {
                *total += value;
            }
            skipped += 1;
        }
        out.rewards = rewards;
        out.reward = reward;
        out.skipped_transition_frames = skipped;
        Ok(out)
    }
}
/// One Python call advances independent environments, with the GIL released.
/// No implicit autoreset: terminal observations are never silently overwritten.
#[pyclass]
pub struct EpisodeBatch {
    pool: rayon::ThreadPool,
    episodes: Vec<Episode>,
    finished: Vec<bool>,
    poisoned: bool,
    skip_non_gameplay: bool,
}
#[pymethods]
impl EpisodeBatch {
    #[new]
    #[pyo3(signature=(rom, task, count, horizon=12000, start=None, skip_non_gameplay=false, contra_solo=false))]
    #[allow(clippy::too_many_arguments)] // Preserve Python positional compatibility; solo is additive.
    fn new(
        py: Python<'_>,
        rom: &[u8],
        task: &str,
        count: usize,
        horizon: u32,
        start: Option<&[u8]>,
        skip_non_gameplay: bool,
        contra_solo: bool,
    ) -> PyResult<Self> {
        if contra_solo && task != "contra-jungle" {
            return Err(error("solo mode requires Contra Jungle"));
        }
        if !(1..=128).contains(&count) {
            return Err(error("batch count must be 1..128"));
        }
        py.detach(|| {
            let threads = count
                .min(8)
                .min(std::thread::available_parallelism().map_or(1, usize::from));
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("nes-training-{i}"))
                .build()
                .map_err(error)?;
            let mut episodes = Vec::with_capacity(count);
            for _ in 0..count {
                episodes.push(if task == "contra-jungle" {
                    if start.is_some() {
                        return Err(error("Contra reset does not accept a state"));
                    }
                    Episode::Contra(Box::new(
                        episode::Episode::new_with_players(
                            rom,
                            horizon,
                            if contra_solo { 1 } else { 2 },
                        )
                        .map_err(error)?,
                    ))
                } else {
                    Episode::Task(Box::new(
                        tasks::Episode::new(rom, game_task(task)?, horizon, start)
                            .map_err(error)?,
                    ))
                });
            }
            Ok(Self {
                pool,
                episodes,
                finished: vec![true; count],
                poisoned: false,
                skip_non_gameplay,
            })
        })
    }
    fn reset(&mut self, py: Python<'_>, warmups: Vec<u32>) -> PyResult<Vec<StepData>> {
        if warmups.len() != self.episodes.len() || warmups.iter().any(|w| *w > 240) {
            return Err(error("one warmup in 0..240 is required per environment"));
        }
        self.poisoned = true;
        let out = py.detach(|| {
            self.pool.install(|| {
                self.episodes
                    .par_iter_mut()
                    .zip(warmups)
                    .map(|(e, w)| e.reset(w))
                    .collect::<PyResult<Vec<_>>>()
            })
        })?;
        self.finished = out.iter().map(|s| s.terminated || s.truncated).collect();
        self.poisoned = false;
        Ok(out)
    }
    fn reset_at(&mut self, py: Python<'_>, index: usize, warmup: u32) -> PyResult<StepData> {
        if self.poisoned {
            return Err(error("batch faulted; reset all environments"));
        }
        if index >= self.episodes.len() || warmup > 240 {
            return Err(error("invalid environment index or warmup"));
        }
        self.poisoned = true;
        let out = py.detach(|| self.episodes[index].reset(warmup))?;
        self.finished[index] = out.terminated || out.truncated;
        self.poisoned = false;
        Ok(out)
    }
    /// Training curriculum entry reached exclusively through normal reset/input.
    /// Reward/death/progress accounting advances with the prefix; it is not reset
    /// at the entry point. An invalid or terminal prefix requires a full reset.
    fn reset_with_prefix_at(
        &mut self,
        py: Python<'_>,
        index: usize,
        warmup: u32,
        prefix: Vec<[u8; 2]>,
    ) -> PyResult<StepData> {
        if self.poisoned {
            return Err(error("batch faulted; reset all environments"));
        }
        if index >= self.episodes.len()
            || warmup > 240
            || prefix.len() > 108_000
            || prefix
                .iter()
                .flatten()
                .any(|m| m & 12 != 0 || m & 48 == 48 || m & 192 == 192)
        {
            return Err(error("invalid curriculum prefix, index or warmup"));
        }
        self.poisoned = true;
        let out = py.detach(|| {
            let episode = &mut self.episodes[index];
            let mut step = episode.reset(warmup)?;
            for masks in prefix {
                if step.terminated || step.truncated {
                    return Err(error("curriculum prefix exceeds the episode"));
                }
                step = episode.step(masks)?;
            }
            if step.terminated || step.truncated {
                return Err(error("curriculum prefix must leave the episode running"));
            }
            Ok(step)
        })?;
        self.finished[index] = false;
        self.poisoned = false;
        Ok(out)
    }
    fn step(&mut self, py: Python<'_>, masks: Vec<[u8; 2]>) -> PyResult<Vec<StepData>> {
        if self.poisoned || self.finished.iter().any(|f| *f) {
            return Err(error(
                "reset finished or faulted environments before stepping",
            ));
        }
        if masks.len() != self.episodes.len()
            || masks
                .iter()
                .flatten()
                .any(|m| m & 12 != 0 || m & 48 == 48 || m & 192 == 192)
        {
            return Err(error(
                "one valid pair of gameplay masks is required per environment",
            ));
        }
        self.poisoned = true;
        let out = py.detach(|| {
            self.pool.install(|| {
                self.episodes
                    .par_iter_mut()
                    .zip(masks)
                    .map(|(e, m)| e.decision_step(m, self.skip_non_gameplay))
                    .collect::<PyResult<Vec<_>>>()
            })
        })?;
        self.finished = out.iter().map(|s| s.terminated || s.truncated).collect();
        self.poisoned = false;
        Ok(out)
    }
    fn snapshot(&self, index: usize) -> PyResult<Vec<u8>> {
        Ok(
            match self
                .episodes
                .get(index)
                .ok_or_else(|| error("invalid environment index"))?
            {
                Episode::Contra(e) => e.machine.save_state(),
                Episode::Task(e) => e.machine.save_state(),
            },
        )
    }
    /// Read the deployed observation contract, including terminal observations,
    /// before Python performs any autoreset. This never advances the machines.
    fn policy_features(
        &self,
        py: Python<'_>,
        schema: &str,
        slots: Vec<usize>,
    ) -> PyResult<Vec<Vec<f32>>> {
        if self.poisoned || slots.len() != self.episodes.len() || slots.iter().any(|s| *s > 1) {
            return Err(error(
                "one valid player per healthy environment is required",
            ));
        }
        let variant = contra_agent::registry::variant(schema)
            .filter(|v| !v.assistance)
            .ok_or_else(|| error("batch features require an autonomous temporal variant"))?;
        py.detach(|| {
            self.pool.install(|| {
                self.episodes
                    .par_iter()
                    .zip(slots)
                    .map(|(e, slot)| {
                        match e {
                            Episode::Contra(e) => {
                                // Critics need transition/terminal state too. Gameplay
                                // actions remain gated by the shared episode runner.
                                let base = e.current().state.observation(slot).map_err(error)?;
                                contra_agent::temporal::extend(
                                    &e.machine,
                                    slot,
                                    &base,
                                    variant.features,
                                )
                                .map_err(error)
                            }
                            Episode::Task(_) => {
                                Err(error("temporal batch features require Contra"))
                            }
                        }
                    })
                    .collect()
            })
        })
    }
}
fn game_task(id: &str) -> PyResult<contra_agent::tasks::Task> {
    contra_agent::tasks::Task::parse(id).map_err(error)
}
