use std::any::Any;
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::sync::{Arc, Mutex};

use notify_rust::Notification as RustNotification;

#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::message::{EngineMsg, Message};

#[cfg(target_os = "linux")]
use notify_rust::NotificationHandle;

/// User-facing notification content shared by every [`Notifier`].
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// Abstraction over a notification delivery mechanism so tests and the
/// future headless server build can substitute a no-op or in-memory
/// implementation.
pub trait Notifier: Send + Sync {
    fn send(&self, notification: &Notification) -> Result<(), String>;
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn as_any(&self) -> &dyn Any;
}

/// Default [`Notifier`] that posts through `notify-rust` (libnotify on
/// Linux, toast on Windows).
pub struct DesktopNotifier;

#[cfg(any(target_os = "linux", target_os = "windows"))]
const OPEN_ACTION: &str = "open";
#[cfg(any(target_os = "linux", target_os = "windows"))]
const REVEAL_ACTION: &str = "reveal";
#[cfg(any(target_os = "linux", target_os = "windows"))]
const RESTART_ACTION: &str = "restart";

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn action_key(a: &NotifyAction) -> Option<&'static str> {
    match a {
        NotifyAction::OpenFile(_) => Some(OPEN_ACTION),
        NotifyAction::RevealDir(_) => Some(REVEAL_ACTION),
        NotifyAction::RestartEngine => Some(RESTART_ACTION),
        NotifyAction::ActivateWindow => Some(OPEN_ACTION),
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn build_keyed(buttons: &[(String, NotifyAction)]) -> Vec<(String, NotifyAction)> {
    buttons
        .iter()
        .filter_map(|(_, a)| action_key(a).map(|k| (k.to_string(), a.clone())))
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn response_to_message(
    response: &notify_rust::NotificationResponse,
    actions: &[(String, NotifyAction)],
    default_action: &NotifyAction,
) -> Option<Message> {
    let target = match response {
        notify_rust::NotificationResponse::Default => Some(default_action.clone()),
        notify_rust::NotificationResponse::Action(key) => actions
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, a)| a.clone())
            .or_else(|| Some(default_action.clone())),
        _ => None,
    };
    target.map(|action| match action {
        NotifyAction::OpenFile(path) => Message::OpenFile(path),
        NotifyAction::RevealDir(path) => Message::RevealDir(path),
        NotifyAction::RestartEngine => Message::Engine(EngineMsg::RestartEngine),
        NotifyAction::ActivateWindow => Message::ActivateWindow,
    })
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
    /// Post `n` with an action button per entry in `buttons`. Each
    /// action's stable key (e.g. `"open"`, `"reveal"`, `"restart"`) is
    /// derived via [`action_key`].
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

/// Bag of [`Notifier`] implementations; the app sends to all of them.
pub struct Notifiers {
    list: Vec<Box<dyn Notifier>>,
}

impl Notifiers {
    /// Create a `Notifiers` containing only [`DesktopNotifier`].
    pub fn new() -> Self {
        Self {
            list: vec![Box::new(DesktopNotifier)],
        }
    }

    #[allow(dead_code)]
    /// Append an extra notifier implementation (e.g. a mock for tests).
    pub fn register(&mut self, n: Box<dyn Notifier>) {
        self.list.push(n);
    }

    #[allow(dead_code)]
    /// Send `n` to every registered notifier; failures are logged but do
    /// not propagate.
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
    let keyed = build_keyed(buttons);
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

#[cfg(target_os = "windows")]
pub fn show(
    notifiers: &Notifiers,
    title: &str,
    body: &str,
    buttons: &[(String, NotifyAction)],
    default_action: NotifyAction,
) -> Option<NotifyEvent> {
    let notification = Notification {
        title: title.to_string(),
        body: body.to_string(),
    };
    let keyed = build_keyed(buttons);
    for notifier in &notifiers.list {
        if let Some(desktop) = notifier.as_any().downcast_ref::<DesktopNotifier>() {
            let mut rust = desktop.build(&notification);
            for (label, action) in buttons {
                if let Some(key) = action_key(action) {
                    rust.action(key, label);
                }
            }
            match rust.show() {
                Ok(handle) => {
                    let wait: Box<
                        dyn FnOnce(&iced::futures::channel::mpsc::Sender<Message>) + Send,
                    > = Box::new(
                        move |sender: &iced::futures::channel::mpsc::Sender<Message>| {
                            let mut s = sender.clone();
                            std::thread::spawn(move || {
                                handle
                                    .wait_for_response(
                                        move |response: &notify_rust::NotificationResponse| {
                                            if let Some(msg) = response_to_message(
                                                response,
                                                &keyed,
                                                &default_action,
                                            ) {
                                                let _ = s.try_send(msg);
                                            }
                                        },
                                    )
                                    .ok();
                            });
                        },
                    );
                    return Some(NotifyEvent { wait });
                }
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

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
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

/// Actions attachable to a notification button or its default body click.
#[derive(Debug, Clone)]
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
pub enum NotifyAction {
    ActivateWindow,
    OpenFile(PathBuf),
    RevealDir(PathBuf),
    RestartEngine,
}

/// In-flight notification that the UI can later subscribe to for button
/// clicks via [`build_notify_stream`].
#[cfg(target_os = "linux")]
pub struct NotifyEvent {
    pub handle: NotificationHandle,
    pub actions: Vec<(String, NotifyAction)>,
    pub default_action: NotifyAction,
}

/// Windows analogue of [`NotifyEvent`]; bundles the closure that owns
/// the `notify-rust` handle and posts translated [`Message`]s back to the
/// GUI.
#[cfg(target_os = "windows")]
pub struct NotifyEvent {
    pub wait: Box<dyn FnOnce(&iced::futures::channel::mpsc::Sender<Message>) + Send>,
}

/// Receiver end of the notification event channel; equality compares the
/// pointer so the same slot always routes to the same stream consumer.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub struct NotifySlot(pub Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<NotifyEvent>>>>);

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl std::hash::Hash for NotifySlot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl PartialEq for NotifySlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl Eq for NotifySlot {}

#[cfg(any(target_os = "linux", target_os = "windows"))]
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
                                if let Some(msg) =
                                    response_to_message(response, &actions, &default_action)
                                {
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

#[cfg(target_os = "windows")]
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
                    (event.wait)(&sender);
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn action_key_mapping() {
        assert_eq!(
            action_key(&NotifyAction::OpenFile(PathBuf::from("/x"))),
            Some("open")
        );
        assert_eq!(
            action_key(&NotifyAction::RevealDir(PathBuf::from("/x"))),
            Some("reveal")
        );
        assert_eq!(action_key(&NotifyAction::RestartEngine), Some("restart"));
        assert_eq!(action_key(&NotifyAction::ActivateWindow), Some("open"));
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn build_keyed_empty() {
        let buttons: Vec<(String, NotifyAction)> = vec![];
        assert!(build_keyed(&buttons).is_empty());
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn build_keyed_preserves_order() {
        let buttons = vec![
            (
                "Open".to_string(),
                NotifyAction::OpenFile(PathBuf::from("/a")),
            ),
            (
                "Reveal".to_string(),
                NotifyAction::RevealDir(PathBuf::from("/b")),
            ),
        ];
        let keyed = build_keyed(&buttons);
        assert_eq!(keyed.len(), 2);
        assert_eq!(keyed[0].0, "open");
        assert_eq!(keyed[1].0, "reveal");
        if let NotifyAction::OpenFile(p) = &keyed[0].1 {
            assert_eq!(p, &PathBuf::from("/a"));
        } else {
            panic!("wrong variant for first keyed action");
        }
    }

    #[test]
    fn notifiers_default_contains_desktop() {
        let ns = Notifiers::new();
        assert_eq!(ns.list.len(), 1);
    }
}
