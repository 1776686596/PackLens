use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::{pick, Language};

mod imp {
    use super::*;

    pub struct Window {
        pub main_paned: gtk::Paned,
        pub sidebar_list: gtk::ListBox,
        pub content_stack: gtk::Stack,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                main_paned: gtk::Paned::new(gtk::Orientation::Horizontal),
                sidebar_list: gtk::ListBox::new(),
                content_stack: gtk::Stack::new(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "PackLensWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();
            window.set_title(Some("PackLens"));
            window.set_default_size(960, 640);
            window.setup_ui();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
                    gtk::Native, gtk::Root, gtk::ShortcutManager,
                    gtk::gio::ActionGroup, gtk::gio::ActionMap;
}

impl Window {
    pub fn new(app: &crate::app::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // 显式启用窗口装饰，标题栏使用 libadwaita 推荐的 ToolbarView + HeaderBar。
        self.set_decorated(true);
        self.set_resizable(true);

        let header_bar = adw::HeaderBar::new();
        header_bar.set_show_start_title_buttons(true);
        header_bar.set_show_end_title_buttons(true);
        header_bar.set_decoration_layout(Some(":minimize,maximize,close"));

        let window_title = gtk::Label::new(None);
        header_bar.set_title_widget(Some(&window_title));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        imp.sidebar_list
            .set_selection_mode(gtk::SelectionMode::Single);
        imp.sidebar_list.add_css_class("navigation-sidebar");

        let nav_rows = vec![
            gtk::Label::new(None),
            gtk::Label::new(None),
            gtk::Label::new(None),
            gtk::Label::new(None),
            gtk::Label::new(None),
        ];
        for row in &nav_rows {
            row.set_halign(gtk::Align::Start);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            row.set_margin_start(12);
            row.set_margin_end(12);
            imp.sidebar_list.append(row);
        }

        let lang_label = gtk::Label::new(None);
        lang_label.set_halign(gtk::Align::Start);

        let lang_selector = gtk::DropDown::from_strings(&["中文", "English"]);
        lang_selector.set_hexpand(true);

        let lang_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        lang_box.set_margin_top(12);
        lang_box.set_margin_bottom(8);
        lang_box.set_margin_start(12);
        lang_box.set_margin_end(12);
        lang_box.append(&lang_label);
        lang_box.append(&lang_selector);

        let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_box.set_size_request(150, -1);
        sidebar_box.append(&lang_box);
        sidebar_box.append(&imp.sidebar_list);

        imp.main_paned.set_wide_handle(true);
        imp.main_paned.set_position(150);
        imp.main_paned.set_resize_start_child(false);
        imp.main_paned.set_resize_end_child(true);
        imp.main_paned.set_shrink_start_child(false);
        imp.main_paned.set_shrink_end_child(true);
        imp.main_paned.set_start_child(Some(&sidebar_box));
        imp.main_paned.set_end_child(Some(&imp.content_stack));

        toolbar_view.set_content(Some(&imp.main_paned));
        self.set_content(Some(&toolbar_view));

        let lang_state = Rc::new(Cell::new(Language::detect_default()));
        lang_selector.set_selected(lang_state.get().to_index());

        let scan_token = Rc::new(RefCell::new(None::<tokio_util::sync::CancellationToken>));

        apply_language(
            self,
            lang_state.get(),
            &lang_label,
            &window_title,
            &imp.sidebar_list,
            &nav_rows,
            &imp.content_stack,
            &scan_token,
        );

        let stack = imp.content_stack.clone();
        imp.sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let page_name = match row.index() {
                    0 => "panorama",
                    1 => "devenv",
                    2 => "disk",
                    3 => "cleanup",
                    4 => "process",
                    _ => return,
                };
                stack.set_visible_child_name(page_name);
            }
        });

        self.connect_close_request({
            let scan_token = scan_token.clone();
            move |_| {
                if let Some(token) = scan_token.borrow_mut().take() {
                    token.cancel();
                }
                glib::Propagation::Proceed
            }
        });

        lang_selector.connect_selected_notify({
            let window = self.clone();
            let lang_label = lang_label.clone();
            let window_title = window_title.clone();
            let sidebar_list = imp.sidebar_list.clone();
            let nav_rows = nav_rows.clone();
            let content_stack = imp.content_stack.clone();
            let scan_token = scan_token.clone();
            let lang_state = lang_state.clone();

            move |dropdown| {
                let lang = Language::from_index(dropdown.selected());
                if lang_state.get() == lang {
                    return;
                }
                lang_state.set(lang);
                apply_language(
                    &window,
                    lang,
                    &lang_label,
                    &window_title,
                    &sidebar_list,
                    &nav_rows,
                    &content_stack,
                    &scan_token,
                );
            }
        });

        if let Some(first) = imp.sidebar_list.row_at_index(0) {
            imp.sidebar_list.select_row(Some(&first));
        }
    }
}

fn apply_language(
    window: &Window,
    lang: Language,
    lang_label: &gtk::Label,
    window_title: &gtk::Label,
    sidebar_list: &gtk::ListBox,
    nav_rows: &[gtk::Label],
    content_stack: &gtk::Stack,
    scan_token: &Rc<RefCell<Option<tokio_util::sync::CancellationToken>>>,
) {
    let title = "PackLens";
    window.set_title(Some(title));
    window_title.set_label(title);
    lang_label.set_label(pick(lang, "语言", "Language"));

    let nav_titles = match lang {
        Language::ZhCn => ["软件全景", "开发环境", "磁盘分析", "清理助手", "进程管理"],
        Language::En => [
            "Software Panorama",
            "Dev Environment",
            "Disk Analysis",
            "Cleanup Assistant",
            "Process Manager",
        ],
    };

    for (label, title) in nav_rows.iter().zip(nav_titles) {
        label.set_label(title);
    }

    if let Some(token) = scan_token.borrow_mut().take() {
        token.cancel();
    }

    if let Some(child) = content_stack.child_by_name("panorama") {
        content_stack.remove(&child);
    }
    if let Some(child) = content_stack.child_by_name("devenv") {
        content_stack.remove(&child);
    }
    if let Some(child) = content_stack.child_by_name("disk") {
        content_stack.remove(&child);
    }
    if let Some(child) = content_stack.child_by_name("cleanup") {
        content_stack.remove(&child);
    }
    if let Some(child) = content_stack.child_by_name("process") {
        content_stack.remove(&child);
    }

    let token = tokio_util::sync::CancellationToken::new();
    let panorama = crate::ui::panorama::build(token.clone(), lang);
    let devenv = crate::ui::devenv::build(token.clone(), lang);
    let disk = crate::ui::disk::build(token.clone(), lang);
    let cleanup = crate::ui::cleanup::build(token.clone(), lang);
    let process = crate::ui::process_manager::build(token.clone(), lang);
    content_stack.add_named(&panorama, Some("panorama"));
    content_stack.add_named(&devenv, Some("devenv"));
    content_stack.add_named(&disk, Some("disk"));
    content_stack.add_named(&cleanup, Some("cleanup"));
    content_stack.add_named(&process, Some("process"));
    *scan_token.borrow_mut() = Some(token);

    if let Some(selected) = sidebar_list.selected_row() {
        let page_name = match selected.index() {
            0 => "panorama",
            1 => "devenv",
            2 => "disk",
            3 => "cleanup",
            4 => "process",
            _ => return,
        };
        content_stack.set_visible_child_name(page_name);
    }
}
