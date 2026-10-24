mod files;

pub(super) use files::around_the_game;
pub(super) use instance_page::{
    ContentAction, content_notice, files_arrived, instance_intent, load_instance, load_section,
    open_instance, open_instance_on,
};

mod instance_page;
