mod handler;
pub(crate) mod repository;
pub use handler::*;

pub(crate) fn service(
    db: &crate::db::Store,
) -> nomail_app::message::MessageService<repository::Store<'_>> {
    nomail_app::message::MessageService {
        store: repository::Store(db),
    }
}
