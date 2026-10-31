pub(crate) mod handler;
pub(crate) mod repository;

pub(crate) fn service(
    db: &sea_orm::DatabaseConnection,
) -> nomail_core::message::MessageService<repository::Store<'_>> {
    nomail_core::message::MessageService {
        store: repository::Store(db),
    }
}
