mod configs;
pub mod parser;
mod receive;

use actix_web::web;
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/webhook/configs")
            .route("", web::get().to(configs::list))
            .route("", web::post().to(configs::create))
            .route("/{id}", web::get().to(configs::get))
            .route("/{id}", web::patch().to(configs::update))
            .route("/{id}", web::delete().to(configs::delete))
            .route("/{id}/enabled", web::patch().to(configs::enabled)),
    )
    .route("/webhook/{key}", web::post().to(receive::handle));
}
