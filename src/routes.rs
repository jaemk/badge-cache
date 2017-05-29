//! Routes
//!  - Map url endpoints to our `handlers`
use router::Router;
use staticfile::Static;
use handlers;


/// Mount our urls and routers on our `Router`
pub fn mount(router: &mut Router) {
    router.get("/", Static::new("static/index.html"), "home");
    router.get("/crates/v/:cratename", handlers::krate, "crate");
    router.get("/badge/:badgeinfo", handlers::badge, "badge");
}
