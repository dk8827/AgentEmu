//! UI-side request generations. Canceling invalidates a response even when the
//! OS file chooser/network transport itself cannot be canceled.
pub struct Pending<T> {
    id: u64,
    pub operation: T,
    pub label: String,
}
pub struct RequestGate<T> {
    serial: u64,
    pending: Option<Pending<T>>,
}
impl<T> Default for RequestGate<T> {
    fn default() -> Self {
        Self {
            serial: 0,
            pending: None,
        }
    }
}
impl<T> RequestGate<T> {
    pub fn begin(&mut self, operation: T, label: &str) -> u64 {
        self.serial = self
            .serial
            .checked_add(1)
            .expect("request generation exhausted");
        self.pending = Some(Pending {
            id: self.serial,
            operation,
            label: label.into(),
        });
        self.serial
    }
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    pub fn pending(&self) -> Option<&Pending<T>> {
        self.pending.as_ref()
    }
    pub fn complete(&mut self, id: u64) -> Option<T> {
        if self.pending.as_ref().is_some_and(|p| p.id == id) {
            self.pending.take().map(|p| p.operation)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn out_of_order_or_duplicate_responses_cannot_replace_the_current_request() {
        let mut requests = RequestGate::default();
        let old = requests.begin("old model", "Downloading");
        let new = requests.begin("new model", "Downloading");
        assert_eq!(requests.complete(old), None);
        assert!(requests.pending().is_some());
        assert_eq!(requests.complete(new), Some("new model"));
        assert_eq!(requests.complete(new), None);
    }
    #[test]
    fn cancel_then_reopen_does_not_accept_the_canceled_file_picker() {
        let mut requests = RequestGate::default();
        let old = requests.begin("game A", "Opening");
        requests.cancel();
        assert_eq!(requests.complete(old), None);
        let new = requests.begin("game B", "Opening");
        assert_ne!(old, new);
        assert_eq!(requests.complete(old), None);
        assert_eq!(requests.complete(new), Some("game B"));
    }
}
