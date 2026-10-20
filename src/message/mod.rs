mod dto;
mod handler;
mod repository;

use actix_web::web;
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/messages")
            .route("", web::get().to(handler::list))
            .route("/unread-count", web::get().to(handler::unread_count))
            .route("/stream", web::get().to(handler::stream))
            .route("/batch", web::post().to(handler::batch))
            .route("/{id}/raw", web::get().to(handler::raw))
            .route("/{id}", web::get().to(handler::get))
            .route("/{id}", web::patch().to(handler::update))
            .route("/{id}", web::delete().to(handler::delete)),
    );
}
