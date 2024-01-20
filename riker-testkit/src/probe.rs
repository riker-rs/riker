pub trait Probe {
    type Msg: Send;
    type Pay: Clone + Send + Sync;
    
    fn event(&self, evt: Self::Msg);
    fn payload(&self) -> &Self::Pay;
}

#[async_trait::async_trait]
pub trait ProbeReceive {
    type Msg: Send;

    async fn recv(&mut self) -> Self::Msg;
    fn reset_timer(&mut self);
    fn last_event_milliseconds(&self) -> u64;
    fn last_event_seconds(&self) -> u64;
}

/// The channel module provides an std::sync::mpsc::channel() based Probe
/// that is suitable for use in a single, local application.
/// This Probe cannot be serialized.
pub mod channel {
    use super::{Probe, ProbeReceive};

    use chrono::prelude::*;

    use tokio::sync::mpsc::{
        channel,
        Sender,
        Receiver,
    };

    pub fn probe<T: Send>() -> (ChannelProbe<(), T>, ChannelProbeReceive<T>) {
        probe_with_payload(())
    }

    pub fn probe_with_payload<P: Clone + Send, T: Send>(payload: P) -> (ChannelProbe<P, T>, ChannelProbeReceive<T>) {
        let (tx, rx) = channel::<T>(100);

        let probe = ChannelProbe {
            payload: Some(payload),
            tx: tx.clone()
        };

        let receiver = ChannelProbeReceive {
            rx: rx,
            tx: tx,
            timer_start: Utc::now()
        };

        (probe, receiver)
    }

    #[derive(Clone, Debug)]
    pub struct ChannelProbe<P, T> {
        payload: Option<P>,
        tx: Sender<T>,
    }

    impl<P, T: std::fmt::Debug + 'static> Probe for ChannelProbe<P, T> 
        where P: Clone + Send + Sync, T: Send {
            type Msg = T;
            type Pay = P;

            fn event(&self, evt: T) {
                let tx = self.clone().tx.clone();
                tokio::spawn(async move {
                    tx.send(evt).await.unwrap();
                });
            }

            fn payload(&self) -> &P {
                &self.payload.as_ref().unwrap()
            }
    }

    impl<P, T: std::fmt::Debug + 'static> Probe for Option<ChannelProbe<P, T>>
        where P: Clone + Send + Sync, T: Send {
            type Msg = T;
            type Pay = P;

            fn event(&self, evt: T) {
                self.as_ref().unwrap().event(evt);
            }

            fn payload(&self) -> &P {
                self.as_ref().unwrap().payload()
            }
    }

    #[allow(dead_code)]
    pub struct ChannelProbeReceive<T> {
        rx: Receiver<T>,
        tx: Sender<T>,
        timer_start: DateTime<Utc>,
    }

    #[async_trait::async_trait]
    impl<T: Send> ProbeReceive for ChannelProbeReceive<T> {
        type Msg = T;

        async fn recv(&mut self) -> T {
            self.rx.recv().await.unwrap()
        }

        fn reset_timer(&mut self) {
            self.timer_start = Utc::now();
        }

        fn last_event_milliseconds(&self) -> u64 {
            let now = Utc::now();
            now.time().signed_duration_since(self.timer_start.time()).num_milliseconds() as u64
        }

        fn last_event_seconds(&self) -> u64 {
            let now = Utc::now();
            now.time().signed_duration_since(self.timer_start.time()).num_seconds() as u64
        }
    }
}


/// Macros that provide easy use of Probes
#[macro_use]
pub mod macros {
    /// Mimicks assert_eq!
    /// Performs an assert_eq! on the first event sent by the probe.
    #[macro_export]
    macro_rules! p_assert_eq {
        ($listen:expr, $expected:expr) => {
            assert_eq!($listen.recv().await, $expected);
        };
    }

    /// Evaluates events sent from the probe with a vector of expected events.
    /// If an unexpected event is received it will assert!(false).
    /// Each good event is removed from the expected vector.
    /// The assertion is complete when there are no more expected events.
    #[macro_export]
    macro_rules! p_assert_events {
        ($listen:expr, $expected:expr) => {
            let mut expected = $expected.clone(); // so we don't need the original mutable
            
            loop {
                let val = $listen.recv().await;
                match expected.iter().position(|x| x == &val) {
                    Some(pos) => {
                        expected.remove(pos);
                        if expected.len() == 0 {
                            break;
                        }
                    }
                    _ => {
                        // probe has received an unexpected event value
                        assert!(false);
                    }
                }
            }
        };
    }

    #[macro_export]
    macro_rules! p_timer {
        ($listen:expr) => {
            $listen.last_event_milliseconds()
        };
    }

    #[cfg(test)]
    mod tests {
        use crate::probe::{Probe, ProbeReceive, channel::probe};

        #[riker_macros::test]
        fn p_assert_eq() {
            let (probe, mut listen) = probe();

            probe.event("test".to_string());
            
            p_assert_eq!(listen, "test".to_string());
        }

        #[riker_macros::test]
        fn p_assert_events() {
            let (probe, mut listen) = probe();

            let expected = vec!["event_1", "event_2", "event_3"];
            probe.event("event_1");
            probe.event("event_2");
            probe.event("event_3");
            
            p_assert_events!(listen, expected);
        }

        #[riker_macros::test]
        fn p_timer() {
            let (probe, listen) = probe();
            probe.event("event_3");
            
            println!("Milliseconds: {}", p_timer!(listen));
        }

    }
}

#[cfg(test)]
mod tests {
    use super::{Probe, ProbeReceive};
    use super::channel::{probe, probe_with_payload};

    #[riker_macros::test]
    fn chan_probe() {
        let (probe, mut listen) = probe();

        probe.event("some event");

        p_assert_eq!(listen, "some event");
    }

    #[riker_macros::test]
    fn chan_probe_with_payload() {
        let payload = "test data".to_string();
        let (probe, mut listen) = probe_with_payload(payload);

        // only event the expected result if the payload is what we expect
        if probe.payload() == "test data" {
            probe.event("data received");
        } else {
            probe.event("");
        }

        p_assert_eq!(listen, "data received");
    }
}
