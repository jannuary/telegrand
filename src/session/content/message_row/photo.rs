use glib::clone;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::{enums::MessageContent, types::File};

use crate::session::chat::{BoxedMessageContent, Message};
use crate::utils::parse_formatted_text;
use crate::Session;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-photo.ui")]
    pub struct MessagePhoto {
        #[template_child]
        pub frame: TemplateChild<gtk::Frame>,
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub caption_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessagePhoto {
        const NAME: &'static str = "ContentMessagePhoto";
        type Type = super::MessagePhoto;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessagePhoto {
        fn constructed(&self, obj: &Self::Type) {
            self.caption_label
                .connect_label_notify(clone!(@weak obj => move |label| {
                    if label.label().len() == 0 {
                        label.set_visible(false);
                        obj.remove_css_class("caption");
                    } else {
                        label.set_visible(true);
                        obj.add_css_class("caption");
                    }
                }));
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.frame.unparent();
            self.caption_label.unparent();
        }
    }

    impl WidgetImpl for MessagePhoto {}
}

glib::wrapper! {
    pub struct MessagePhoto(ObjectSubclass<imp::MessagePhoto>)
        @extends gtk::Widget;
}

impl Default for MessagePhoto {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagePhoto {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessagePhoto")
    }

    fn session(&self) -> Session {
        self.ancestor(Session::static_type())
            .expect("Expected Session as ancestor")
            .downcast()
            .unwrap()
    }

    fn download_image(&self, file_id: i32) {
        let (sender, receiver) = glib::MainContext::sync_channel::<File>(Default::default(), 5);

        receiver.attach(
            None,
            clone!(@weak self as obj => @default-return glib::Continue(false), move |file| {
                let self_ = imp::MessagePhoto::from_instance(&obj);

                if file.local.is_downloading_completed {
                    self_.picture.set_filename(&file.local.path);
                }

                glib::Continue(true)
            }),
        );

        self.session().download_file(file_id, sender);
    }

    pub fn set_message(&self, message: &Message) {
        let self_ = imp::MessagePhoto::from_instance(self);

        if message.is_outgoing() {
            self.add_css_class("outgoing");
        } else {
            self.remove_css_class("outgoing");
        }

        if let MessageContent::MessagePhoto(data) = message.content().0 {
            self.set_hexpand(true);

            if let Some(photo_size) = data.photo.sizes.last() {
                if photo_size.photo.local.is_downloading_completed {
                    self_.picture.set_filename(&photo_size.photo.local.path);
                } else {
                    self.download_image(photo_size.photo.id);
                }
            }

            // Caption label
            let content_expression = gtk::PropertyExpression::new(
                Message::static_type(),
                gtk::NONE_EXPRESSION,
                "content",
            );
            let caption_expression = gtk::ClosureExpression::new(
                move |args| -> String {
                    let content = args[1].get::<BoxedMessageContent>().unwrap();
                    if let MessageContent::MessagePhoto(data) = content.0 {
                        parse_formatted_text(data.caption)
                    } else {
                        unreachable!("Unexpected message content type: {:?}", content.0);
                    }
                },
                &[content_expression.upcast()],
            );
            caption_expression.bind(&*self_.caption_label, "label", Some(message));
        }
    }
}
