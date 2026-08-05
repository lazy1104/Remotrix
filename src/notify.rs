use std::any::Any;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};

use notify_rust::Notification as RustNotification;

#[cfg(target_os = "linux")]
use crate::message::Message;

#[cfg(target_os = "linux")]
use notify_rust::NotificationHandle;

pub struct Notification {
    pub title: String,
    pub body: String,
}

pub trait Notifier: Send + Sync {
    fn send(&self, notification: &Notification) -> Result<(), String>;
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn as_any(&self) -> &dyn Any;
}

pub struct DesktopNotifier;

#[cfg(target_os = "linux")]
const OPEN_ACTION: &str = "open";
#[cfg(target_os = "linux")]
const REVEAL_ACTION: &str = "reveal";

#[cfg(target_os = "linux")]
fn action_key(a: &NotifyAction) -> Option<&'static str> {
    match a {
        NotifyAction::OpenFile(_) => Some(OPEN_ACTION),
        NotifyAction::RevealDir(_) => Some(REVEAL_ACTION),
        NotifyAction::ActivateWindow => Some(OPEN_ACTION),
    }
}

impl DesktopNotifier {
    fn build(&self, n: &Notification) -> RustNotification {
        let mut notification = RustNotification::new();
        notification
            .appname(crate::APP_ID)
            .summary(&n.title)
            .body(&n.body);
        #[cfg(target_os = "linux")]
        notification.hint(notify_rust::Hint::DesktopEntry(crate::APP_ID.to_string()));
        #[cfg(target_os = "windows")]
        {
            crate::win_toast::ensure_shortcut();
            notification.app_id(crate::win_toast::AUMID);
        }
        notification.finalize()
    }
}

impl Notifier for DesktopNotifier {
    fn send(&self, notification: &Notification) -> Result<(), String> {
        self.build(notification).show().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(target_os = "linux")]
impl DesktopNotifier {
    pub fn show_with_actions(
        &self,
        n: &Notification,
        buttons: &[(String, NotifyAction)],
    ) -> Result<NotificationHandle, String> {
        let mut notification = self.build(n);
        for (label, action) in buttons {
            if let Some(key) = action_key(action) {
                notification.action(key, label);
            }
        }
        notification.show().map_err(|e| e.to_string())
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

#[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
pub fn show(notifiers: &Notifiers, title: &str, body: &str, buttons: &[(String, NotifyAction)]) {
    let _ = buttons;
    let notification = Notification {
        title: title.to_string(),
        body: body.to_string(),
    };
    for notifier in &notifiers.list {
        if let Err(e) = notifier.send(&notification) {
            tracing::warn!(error = %e, "notification send failed");
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum NotifyAction {
    ActivateWindow,
    OpenFile(PathBuf),
    RevealDir(PathBuf),
}

#[cfg(target_os = "linux")]
pub struct NotifyEvent {
    pub handle: NotificationHandle,
    pub actions: Vec<(String, NotifyAction)>,
    pub default_action: NotifyAction,
}

#[cfg(target_os = "linux")]
pub struct NotifySlot(pub Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<NotifyEvent>>>>);

#[cfg(target_os = "linux")]
impl std::hash::Hash for NotifySlot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

#[cfg(target_os = "linux")]
impl PartialEq for NotifySlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(target_os = "linux")]
impl Eq for NotifySlot {}

#[cfg(target_os = "linux")]
impl Clone for NotifySlot {
    fn clone(&self) -> Self {
        NotifySlot(self.0.clone())
    }
}

#[cfg(target_os = "linux")]
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
                        .wait_for_action_async(
                            move |response: &notify_rust::NotificationResponse| {
                                let target = match response {
                                    notify_rust::NotificationResponse::Default => {
                                        Some(default_action.clone())
                                    }
                                    notify_rust::NotificationResponse::Action(key) => actions
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
                            },
                        )
                        .await;
                }
            }
        },
    )
}
