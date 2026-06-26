use std::{
    io,
    sync::mpsc::{self, TryRecvError},
    thread,
};

use crate::provider_config;

pub(super) type ModelFetchResult = Result<Vec<String>, String>;

pub(super) enum ModelFetchStatus {
    Pending,
    Disconnected,
    Finished {
        base_url: String,
        result: ModelFetchResult,
    },
}

pub(super) struct ModelFetchTask {
    base_url: String,
    receiver: mpsc::Receiver<ModelFetchResult>,
}

impl ModelFetchTask {
    pub(super) fn spawn(base_url: String, api_key: String) -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let request_base_url = base_url.clone();
        thread::Builder::new()
            .name("codex-board-model-fetch".to_string())
            .spawn(move || {
                let result = provider_config::fetch_provider_models(&base_url, &api_key)
                    .map_err(|err| err.to_string());
                let _ = sender.send(result);
            })?;

        Ok(Self {
            base_url: request_base_url,
            receiver,
        })
    }

    pub(super) fn poll(&self) -> ModelFetchStatus {
        match self.receiver.try_recv() {
            Ok(result) => ModelFetchStatus::Finished {
                base_url: self.base_url.clone(),
                result,
            },
            Err(TryRecvError::Empty) => ModelFetchStatus::Pending,
            Err(TryRecvError::Disconnected) => ModelFetchStatus::Disconnected,
        }
    }

    #[cfg(test)]
    pub(super) const fn from_receiver(
        base_url: String,
        receiver: mpsc::Receiver<ModelFetchResult>,
    ) -> Self {
        Self { base_url, receiver }
    }
}
