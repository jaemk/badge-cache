//! Routes
//!  - Map url endpoints to our `handlers`
use router::Router;
use staticfile::Static;
use handlers::Handlers;


/// Mount our urls and routers on our `Router`
pub fn mount(router: &mut Router, handlers: &Handlers) {
    router.get("/",                     Static::new("static/index.html"),   "home");
    router.get("/crates/v/:cratename",  handlers.badge_handler.clone(),     "crates");
    router.get("/crate/:cratename",     handlers.badge_handler.clone(),     "crate");
    router.get("/badge/:badgeinfo",     handlers.badge_handler.clone(),     "badge");
}
