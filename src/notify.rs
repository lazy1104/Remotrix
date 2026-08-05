use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notify_rust::{
    Hint, Notification as RustNotification, NotificationHandle, NotificationResponse,
};

use crate::message::Message;

pub struct Notification {
    pub title: String,
    pub body: String,
}

pub trait Notifier: Send + Sync {
    fn send(&self, notification: &Notification) -> Result<(), String>;
    fn as_any(&self) -> &dyn Any;
}

pub struct DesktopNotifier;

const OPEN_ACTION: &str = "open";
const REVEAL_ACTION: &str = "reveal";

fn action_key(a: &NotifyAction) -> Option<&'static str> {
    match a {
        NotifyAction::OpenFile(_) => Some(OPEN_ACTION),
        NotifyAction::RevealDir(_) => Some(REVEAL_ACTION),
        NotifyAction::ActivateWindow => Some(OPEN_ACTION),
    }
}

impl DesktopNotifier {
    pub fn show_with_actions(
        &self,
        n: &Notification,
        buttons: &[(String, NotifyAction)],
    ) -> Result<NotificationHandle, String> {
        self.build(n, buttons).show().map_err(|e| e.to_string())
    }

    fn build(&self, n: &Notification, buttons: &[(String, NotifyAction)]) -> RustNotification {
        let mut notification = RustNotification::new();
        notification
            .appname(crate::APP_ID)
            .hint(Hint::DesktopEntry(crate::APP_ID.to_string()))
            .summary(&n.title)
            .body(&n.body);
        for (label, action) in buttons {
            if let Some(key) = action_key(action) {
                notification.action(key, label);
            }
        }
        notification.finalize()
    }
}

impl Notifier for DesktopNotifier {
    fn send(&self, notification: &Notification) -> Result<(), String> {
        self.build(notification, &[])
            .show()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct Notifiers {
    list: Vec<Box<dyn Notifier>>,
}

impl Notifiers {
    pub fn new() -> Self {
        Self {
            list: vec![Box::new(DesktopNotifier)],
        }
    }

    #[allow(dead_code)]
    pub fn register(&mut self, n: Box<dyn Notifier>) {
        self.list.push(n);
    }

    #[allow(dead_code)]
    pub fn send_all(&self, n: &Notification) {
        for notifier in &self.list {
            if let Err(e) = notifier.send(n) {
                tracing::warn!(error = %e, "notification send failed");
            }
        }
    }
}

impl Default for Notifiers {
    fn default() -> Self {
        Self::new()
    }
}

pub fn show(
    notifiers: &Notifiers,
    title: &str,
    body: &str,
    buttons: &[(String, NotifyAction)],
) -> Option<(NotificationHandle, Vec<(String, NotifyAction)>)> {
    let notification = Notification {
        title: title.to_string(),
        body: body.to_string(),
    };
    let keyed: Vec<(String, NotifyAction)> = buttons
        .iter()
        .filter_map(|(_, a)| action_key(a).map(|k| (k.to_string(), a.clone())))
        .collect();
    for notifier in &notifiers.list {
        if let Some(desktop) = notifier.as_any().downcast_ref::<DesktopNotifier>() {
            match desktop.show_with_actions(&notification, buttons) {
                Ok(handle) => return Some((handle, keyed)),
                Err(e) => {
                    tracing::warn!(error = %e, "wake notification failed");
                    let _ = desktop.send(&notification);
                }
            }
        } else {
            let _ = notifier.send(&notification);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub enum NotifyAction {
    ActivateWindow,
    OpenFile(PathBuf),
    RevealDir(PathBuf),
}

pub struct NotifyEvent {
    pub handle: NotificationHandle,
    pub actions: Vec<(String, NotifyAction)>,
    pub default_action: NotifyAction,
}

pub struct NotifySlot(pub Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<NotifyEvent>>>>);

impl std::hash::Hash for NotifySlot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for NotifySlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for NotifySlot {}

impl Clone for NotifySlot {
    fn clone(&self) -> Self {
        NotifySlot(self.0.clone())
    }
}

pub fn build_notify_stream(slot: &NotifySlot) -> impl iced::futures::Stream<Item = Message> {
    let rx = {
        let mut guard = slot.0.lock().expect("notify slot poisoned");
        guard.take()
    };
    iced::stream::channel(
        1,
        move |sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            if let Some(mut rx) = rx {
                while let Some(event) = rx.recv().await {
                    let mut s = sender.clone();
                    let default_action = event.default_action;
                    let actions = event.actions;
                    event
                        .handle
                        .wait_for_action_async(move |response: &NotificationResponse| {
                            let target = match response {
                                NotificationResponse::Default => Some(default_action.clone()),
                                NotificationResponse::Action(key) => actions
                                    .iter()
                                    .find(|(k, _)| k == key)
                                    .map(|(_, a)| a.clone())
                                    .or_else(|| Some(default_action.clone())),
                                _ => None,
                            };
                            if let Some(action) = target {
                                let msg = match action {
                                    NotifyAction::OpenFile(path) => Message::OpenFile(path),
                                    NotifyAction::RevealDir(path) => Message::RevealDir(path),
                                    NotifyAction::ActivateWindow => Message::ActivateWindow,
                                };
                                let _ = s.try_send(msg);
                            }
                        })
                        .await;
                }
            }
        },
    )
}
