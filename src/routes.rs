//! Routes
//!  - Map url endpoints to our `handlers`
use router::Router;
use staticfile::Static;
use handlers::{self, Handlers};


/// Mount our urls and routers on our `Router`
pub fn mount(router: &mut Router, handlers: &Handlers) {
    router.get("/crates/v/:cratename",  handlers.badge_handler.clone(), "crates");
    router.get("/crate/:cratename",     handlers.badge_handler.clone(), "crate");
    router.get("/badge/:badgeinfo",     handlers.badge_handler.clone(), "badge");

    router.post("/reset/crates/v/:cratename",   handlers.reset_badge_handler.clone(), "reset_crates");
    router.post("/reset/crate/:cratename",      handlers.reset_badge_handler.clone(), "reset_crate");
    router.post("/reset/badge/:badgeinfo",      handlers.reset_badge_handler.clone(), "reset_badge");
    router.get("/reset",                        handlers::reset_page,                 "reset");

    router.get("/robots.txt",           Static::new("static/robots.txt"), "robots");
    router.get("/",                     handlers::landing, "home");
}
