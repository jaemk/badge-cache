//! Routes
//!  - Map url endpoints to our `handlers`
use router::Router;
use handlers;


/// Mount our urls and routers on our `Router`
pub fn mount(router: &mut Router) {
    router.get("/", handlers::home, "home");
    router.get("/crate/:cratename", handlers::krate, "crate");
    router.get("/badge/:badgeinfo", handlers::badge, "badge");
}
