use crate::tuliprox_error::{TuliProxError, TuliProxErrorKind, handle_tuliprox_error_result_list};
use crate::model::ConfigInput;
use crate::model::config_target::ConfigTarget;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSource {
    pub inputs: Vec<ConfigInput>,
    pub targets: Vec<ConfigTarget>,
}

impl ConfigSource {
    #[allow(clippy::cast_possible_truncation)]
    pub fn prepare(&mut self, index: u16, include_computed: bool) -> Result<u16, TuliProxError> {
        handle_tuliprox_error_result_list!(TuliProxErrorKind::Info, self.inputs.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+(idx as u16), include_computed)));
        Ok(index + (self.inputs.len() as u16))
    }

    pub fn get_inputs_for_target(&self, target_name: &str) -> Option<Vec<&ConfigInput>> {
        for target in &self.targets {
            if target.name.eq(target_name) {
                let inputs = self.inputs.iter().filter(|&i| i.enabled).collect::<Vec<&ConfigInput>>();
                if !inputs.is_empty() {
                    return Some(inputs);
                }
            }
        }
        None
    }
}