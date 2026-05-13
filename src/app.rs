use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Application;

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "PackLensApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            let app = self.obj();
            let window = crate::window::Window::new(&app);
            window.present();
        }

        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();

            let quit = gio::ActionEntry::builder("quit")
                .activate(|app: &super::Application, _, _| app.quit())
                .build();
            app.add_action_entries([quit]);
            app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", "io.github.packlens.PackLens")
            .build()
    }
}
