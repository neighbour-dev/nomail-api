mod handler;
pub(crate) mod repository;
pub use handler::*;

pub(crate) fn service(
    db: &crate::db::Store,
) -> nomail_core::message::MessageService<repository::Store<'_>> {
    nomail_core::message::MessageService {
        store: repository::Store(db),
    }
}
