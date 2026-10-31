pub(crate) mod handler;
pub(crate) mod repository;

pub(crate) fn service(
    db: &sea_orm::DatabaseConnection,
) -> nomail_app::message::MessageService<repository::Store<'_>> {
    nomail_app::message::MessageService {
        store: repository::Store(db),
    }
}
