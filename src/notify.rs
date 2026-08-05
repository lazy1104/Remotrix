use std::any::Any;
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

impl DesktopNotifier {
    pub fn show_with_action(&self, n: &Notification) -> Result<NotificationHandle, String> {
        self.build(n).show().map_err(|e| e.to_string())
    }

    fn build(&self, n: &Notification) -> RustNotification {
        RustNotification::new()
            .appname(crate::APP_ID)
            .hint(Hint::DesktopEntry(crate::APP_ID.to_string()))
            .summary(&n.title)
            .body(&n.body)
            .finalize()
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

pub fn show_wake(notifiers: &Notifiers, title: &str, body: &str) -> Option<NotificationHandle> {
    let notification = Notification {
        title: title.to_string(),
        body: body.to_string(),
    };
    for notifier in &notifiers.list {
        if let Some(desktop) = notifier.as_any().downcast_ref::<DesktopNotifier>() {
            match desktop.show_with_action(&notification) {
                Ok(handle) => return Some(handle),
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

pub struct NotifySlot(
    pub Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<NotificationHandle>>>>,
);

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
                while let Some(handle) = rx.recv().await {
                    let mut s = sender.clone();
                    handle
                        .wait_for_action_async(move |action: &NotificationResponse| {
                            if matches!(action, NotificationResponse::Default) {
                                let _ = s.try_send(Message::ActivateWindow);
                            }
                        })
                        .await;
                }
            }
        },
    )
}
